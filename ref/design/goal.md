Bidirectional synchronization between TaskWarrior and CalDAV servers.


features wanted : 

     Bidirectional Sync - Changes propagate both ways (TaskWarrior ↔ CalDAV).
     No Sync Database - CalDAV UID is stored as a TaskWarrior UDA, so there is no need for an intermediate sync database.
     Multi-Client Support - Multiple TaskWarrior instances can sync against the same CalDAV server.
     Project Mapping - One TaskWarrior project maps to one CalDAV calendar.
     LWW Conflict Resolution - Timestamp-based conflict resolution. Last Write Wins.
     Dry Run Mode - Preview changes before syncing.
     Comprehensive Tests - Extensive unit tests, plus integration tests performed in Docker to replicate actual usage.
