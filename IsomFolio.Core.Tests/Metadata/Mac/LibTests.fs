module IsomFolio.Core.Tests.Metadata.Mac.LibTests

open System.Diagnostics
open System.IO
open Xunit
open IsomFolio.Core.Metadata.Mac

module AppleMetadataTests =
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
        
        let metadata = AppleMetadata.fromFilePath "Resources/temp/white16.png"
        Assert.Equivalent([ { Text = "test_tag"; ColorIdx = 0 } ], metadata.UserTags)
