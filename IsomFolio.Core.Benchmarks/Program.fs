module IsomFolio.Core.Benchmarks.Program

open System
open System.IO
open BenchmarkDotNet.Attributes
open BenchmarkDotNet.Running
open IsomFolio.Core.Indexing
open FSharp.Control

/// Set ISOMFOLIO_BENCH_DIR to a folder of image files before running.
/// A download script will be provided to populate a representative dataset.
[<MemoryDiagnoser>]
type ScannerBenchmarks() =
    let mutable benchDir = ""

    [<Params(4, 8, 16)>]
    member val Parallelism = 4 with get, set

    [<GlobalSetup>]
    member _.Setup() =
        match Environment.GetEnvironmentVariable("ISOMFOLIO_BENCH_DIR") with
        | null | "" ->
            failwith "ISOMFOLIO_BENCH_DIR is not set — point it at a folder of image files"
        | path when not (Directory.Exists(path)) ->
            failwith $"ISOMFOLIO_BENCH_DIR={path} does not exist"
        | path ->
            benchDir <- path

    [<Benchmark(Baseline = true)>]
    member _.Sequential() =
        Scanner.enumerateFiles (Scanner.runSequential Scanner.defaultJob) 500 benchDir
        |> TaskSeq.iter (fun _ -> ())
        |> Async.AwaitTask
        |> Async.RunSynchronously

    [<Benchmark>]
    member this.Parallel() =
        Scanner.enumerateFiles (Scanner.runParallel this.Parallelism Scanner.defaultJob) 500 benchDir
        |> TaskSeq.iter (fun _ -> ())
        |> Async.AwaitTask
        |> Async.RunSynchronously

[<EntryPoint>]
let main _ =
    BenchmarkRunner.Run<ScannerBenchmarks>() |> ignore
    0
