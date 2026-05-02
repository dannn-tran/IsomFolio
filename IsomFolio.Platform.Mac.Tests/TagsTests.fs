module TagsTests

open System.Diagnostics
open System.IO
open IsomFolio.Mac
open Xunit

module GetMacTagsTests =
    let private unpackTarPreservingXattrs (tarPath: string) (destDir: string) =
        Directory.CreateDirectory(destDir) |> ignore
        let p = Process.Start(ProcessStartInfo(
            FileName = "tar",
            Arguments = $"-xpf \"{tarPath}\" -C \"{destDir}\"",
            UseShellExecute = false
        ))
        p.WaitForExit()
        if p.ExitCode <> 0 then
            failwith $"tar exited with code {p.ExitCode}"
            
    [<Fact>]
    let ``Read macOS tags from a real file`` () =
        unpackTarPreservingXattrs "Resources/white16_test_tag.tar.gz" "Resources/temp"
        
        match Tags.extractTags "Resources/temp/white16.png" with 
        | Ok tags -> Assert.Equivalent(["test_tag"], tags)
        | Error e -> Assert.Fail(e.ToString())
