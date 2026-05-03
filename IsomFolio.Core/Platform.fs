module IsomFolio.Core.Platform

open System.Runtime.InteropServices

type OS = Windows | Linux | MacOS | Unknown

let currentOS =
    if   RuntimeInformation.IsOSPlatform(OSPlatform.Windows) then Windows
    elif RuntimeInformation.IsOSPlatform(OSPlatform.Linux)   then Linux
    elif RuntimeInformation.IsOSPlatform(OSPlatform.OSX)     then MacOS
    else Unknown