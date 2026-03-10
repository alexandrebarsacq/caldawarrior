use std::collections::HashMap;

use uuid::Uuid;

use crate::types::{IREntry, Warning};

/// Resolves TW dependency UUIDs to CalDAV UIDs and detects cyclic dependencies.
///
/// For each `IREntry`, the resolver:
/// 1. Maps each TW dependency UUID to the corresponding CalDAV UID and
///    stores the result in `entry.resolved_depends`.
/// 2. Emits an `UnresolvableDependency` warning when a TW dependency UUID is
///    not present in the IR or the matched entry has no CalDAV UID.
/// 3. Detects cycles in the dependency graph via an iterative DFS and marks
///    every node in a cycle with `entry.cyclic = true`, emitting a `CyclicEntry`
///    warning for each such node.
///
/// Returns the list of non-fatal warnings accumulated during resolution.
pub fn resolve_dependencies(entries: &mut Vec<IREntry>) -> Vec<Warning> {
    // --- Step 1: build tw_uuid -> index map for O(1) lookups ---
    // Only include entries that have a tw_uuid (CalDAV-only terminal entries may have None).
    let index: HashMap<Uuid, usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.tw_uuid.map(|uuid| (uuid, i)))
        .collect();

    let mut warnings: Vec<Warning> = Vec::new();

    // --- Step 2: resolve dependency UIDs ---
    for i in 0..entries.len() {
        let depends: Vec<Uuid> = entries[i]
            .tw_task
            .as_ref()
            .map(|t| t.depends.clone())
            .unwrap_or_default();

        let mut resolved: Vec<String> = Vec::new();

        for dep_uuid in &depends {
            match index.get(dep_uuid) {
                Some(&j) => match &entries[j].caldav_uid {
                    Some(uid) => resolved.push(uid.clone()),
                    None => warnings.push(Warning {
                        tw_uuid: Some(*dep_uuid),
                        message: format!(
                            "UnresolvableDependency: TW UUID {} has no CalDAV UID",
                            dep_uuid
                        ),
                    }),
                },
                None => warnings.push(Warning {
                    tw_uuid: Some(*dep_uuid),
                    message: format!(
                        "UnresolvableDependency: TW UUID {} not found in IR",
                        dep_uuid
                    ),
                }),
            }
        }

        entries[i].resolved_depends = resolved;
    }

    // --- Step 3: DFS cycle detection ---
    // Build adjacency list using resolved index positions (TW-only edges).
    let n = entries.len();
    let adj: Vec<Vec<usize>> = entries
        .iter()
        .map(|e| {
            e.tw_task
                .as_ref()
                .map(|t| {
                    t.depends
                        .iter()
                        .filter_map(|uuid| index.get(uuid).copied())
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    // Three-colour DFS: 0 = white (unvisited), 1 = gray (in stack), 2 = black (done).
    let mut color = vec![0u8; n];
    let mut cyclic_nodes = vec![false; n];

    for start in 0..n {
        if color[start] != 0 {
            continue;
        }

        // Stack entries: (node_index, next_adjacency_index_to_visit)
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        color[start] = 1;

        while let Some((node, adj_idx)) = stack.last_mut() {
            let node = *node;
            if *adj_idx < adj[node].len() {
                let next = adj[node][*adj_idx];
                *adj_idx += 1;

                match color[next] {
                    1 => {
                        // Back-edge: found a cycle. Mark every node on the stack
                        // from `next`'s position through the current node.
                        let cycle_start = stack
                            .iter()
                            .position(|(n, _)| *n == next)
                            .unwrap_or(0);
                        for &(cyclic_node, _) in &stack[cycle_start..] {
                            cyclic_nodes[cyclic_node] = true;
                        }
                        cyclic_nodes[node] = true;
                    }
                    0 => {
                        color[next] = 1;
                        stack.push((next, 0));
                    }
                    _ => {} // already fully explored
                }
            } else {
                // All neighbours visited; finish this node.
                color[node] = 2;
                stack.pop();
            }
        }
    }

    // Emit CyclicEntry warnings and set the flag on affected entries.
    for i in 0..n {
        if cyclic_nodes[i] {
            entries[i].cyclic = true;
            let desc = entries[i]
                .tw_task
                .as_ref()
                .map(|t| t.description.as_str())
                .unwrap_or("<no description>");
            warnings.push(Warning {
                tw_uuid: entries[i].tw_uuid,
                message: format!(
                    "CyclicEntry: task '{}' is part of a dependency cycle",
                    desc
                ),
            });
        }
    }

    warnings
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{IREntry, TWTask};
    use chrono::Utc;
    use uuid::Uuid;

    fn make_entry(tw_uuid: Uuid, caldav_uid: Option<&str>, depends: Vec<Uuid>) -> IREntry {
        let tw_task = Some(TWTask {
            uuid: tw_uuid,
            status: "pending".to_string(),
            description: format!("task-{}", tw_uuid),
            entry: Utc::now(),
            modified: None,
            due: None,
            scheduled: None,
            wait: None,
            until: None,
            end: None,
            caldavuid: None,
            priority: None,
            project: None,
            tags: None,
            recur: None,
            urgency: None,
            id: None,
            depends,
            annotations: vec![],
        });
        IREntry {
            tw_uuid: Some(tw_uuid),
            caldav_uid: caldav_uid.map(str::to_owned),
            tw_task,
            fetched_vtodo: None,
            resolved_depends: vec![],
            cyclic: false,
            calendar_url: None,
            dirty_tw: false,
            dirty_caldav: false,
            project: None,
        }
    }

    #[test]
    fn test_simple_dependency() {
        let uuid_a = Uuid::new_v4();
        let uuid_b = Uuid::new_v4();
        let caldav_b = "caldav-uid-b";

        let mut entries = vec![
            make_entry(uuid_a, Some("caldav-uid-a"), vec![uuid_b]),
            make_entry(uuid_b, Some(caldav_b), vec![]),
        ];

        let warnings = resolve_dependencies(&mut entries);

        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
        assert_eq!(entries[0].resolved_depends, vec![caldav_b.to_string()]);
        assert_eq!(entries[1].resolved_depends, Vec::<String>::new());
        assert!(!entries[0].cyclic);
        assert!(!entries[1].cyclic);
    }

    #[test]
    fn test_cycle_detection() {
        let uuid_a = Uuid::new_v4();
        let uuid_b = Uuid::new_v4();

        let mut entries = vec![
            make_entry(uuid_a, Some("caldav-uid-a"), vec![uuid_b]),
            make_entry(uuid_b, Some("caldav-uid-b"), vec![uuid_a]),
        ];

        let warnings = resolve_dependencies(&mut entries);

        // Both entries must be marked cyclic.
        assert!(entries[0].cyclic, "entry A should be cyclic");
        assert!(entries[1].cyclic, "entry B should be cyclic");

        // Two CyclicEntry warnings should be emitted (one per node).
        let cyclic_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.message.contains("CyclicEntry"))
            .collect();
        assert_eq!(cyclic_warnings.len(), 2);
    }

    #[test]
    fn test_unresolvable_uuid_not_in_ir() {
        let uuid_a = Uuid::new_v4();
        let uuid_missing = Uuid::new_v4();

        let mut entries = vec![make_entry(uuid_a, Some("caldav-uid-a"), vec![uuid_missing])];

        let warnings = resolve_dependencies(&mut entries);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("UnresolvableDependency"));
        assert!(warnings[0].message.contains("not found in IR"));
        assert_eq!(warnings[0].tw_uuid, Some(uuid_missing));
        assert!(entries[0].resolved_depends.is_empty());
    }

    #[test]
    fn test_unresolvable_no_caldav_uid() {
        let uuid_a = Uuid::new_v4();
        let uuid_b = Uuid::new_v4();

        // B exists in IR but has no CalDAV UID (TW-only entry).
        let mut entries = vec![
            make_entry(uuid_a, Some("caldav-uid-a"), vec![uuid_b]),
            make_entry(uuid_b, None, vec![]),
        ];

        let warnings = resolve_dependencies(&mut entries);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("UnresolvableDependency"));
        assert!(warnings[0].message.contains("no CalDAV UID"));
        assert!(entries[0].resolved_depends.is_empty());
    }

    #[test]
    fn test_no_dependencies() {
        let uuid_a = Uuid::new_v4();
        let mut entries = vec![make_entry(uuid_a, Some("caldav-uid-a"), vec![])];

        let warnings = resolve_dependencies(&mut entries);

        assert!(warnings.is_empty());
        assert!(entries[0].resolved_depends.is_empty());
        assert!(!entries[0].cyclic);
    }
}
