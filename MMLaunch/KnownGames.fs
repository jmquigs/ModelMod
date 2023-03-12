namespace MMLaunch

module KnownGames =

    type DllPath =
    | D3D9 of string
    | D3D11 of string

    type KnownGame = {
        ExeBaseName: string
        D3DPaths: DllPath list
        Is64Bit: bool
    }

    let AllKnownGames = [
        { 
            KnownGame.ExeBaseName = "gw2-64"
            D3DPaths = [D3D9(@"bin64"); D3D11(@"")]
            Is64Bit = true
        }
        { 
            KnownGame.ExeBaseName = "gw"
            D3DPaths = [D3D9(@"")]
            Is64Bit = false
        }
        {
            KnownGame.ExeBaseName = "TESV"
            D3DPaths = [D3D9(@"")] // not sure about this
            Is64Bit = false
        }
        {
            KnownGame.ExeBaseName = "DragonAge2"
            D3DPaths = [D3D9(@"")] // not sure about this
            Is64Bit = false
        }
    ]
