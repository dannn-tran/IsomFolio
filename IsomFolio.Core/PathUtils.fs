module IsomFolio.PathUtils

open System
open System.IO

let normalizePath (path: string) =
    if String.IsNullOrWhiteSpace path then path
    else
        let full = Path.GetFullPath(path)
        let trimmed = full.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar)
        
        // On Windows and macOS, paths are case-insensitive. 
        // To ensure stable FileIds, we lowercase the path during normalization.
        if OperatingSystem.IsWindows() || OperatingSystem.IsMacOS() then
            trimmed.ToLowerInvariant()
        else
            trimmed

let samePath (left: string) (right: string) =
    String.Equals(left, right, StringComparison.Ordinal)

let isDescendantPath (ancestor: string) (candidate: string) =
    if samePath ancestor candidate then
        false
    else
        let prefix = ancestor + string Path.DirectorySeparatorChar
        candidate.StartsWith(prefix, StringComparison.Ordinal)
