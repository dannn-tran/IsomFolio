module IsomFolio.Platform.Mac.Tags

let getMacTags (filePath: string) : Result<string list, string> =
    try
        let psi = System.Diagnostics.ProcessStartInfo("mdls", $"-name kMDItemUserTags -raw \"{filePath}\"")
        psi.RedirectStandardOutput <- true
        psi.UseShellExecute <- false
        let p = System.Diagnostics.Process.Start(psi)
        let output = p.StandardOutput.ReadToEnd()
        p.WaitForExit()
        
        if output = "(null)" then
            Ok []
        else
            output.Trim('(', ')', '\n').Split(',')
            |> Array.map (fun s -> s.Trim().Trim('"'))
            |> Array.filter (fun s -> s <> "")
            |> Array.toList
            |> Ok
    with ex ->
        Error ex.Message