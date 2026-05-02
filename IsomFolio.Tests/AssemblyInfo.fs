module IsomFolio.Tests.AssemblyInfo

// xUnit integration tests share a global SQLite connection via Db.conn.
// Disable parallel execution to prevent cross-test interference.
[<assembly: Xunit.CollectionBehavior(DisableTestParallelization = true)>]
do ()
