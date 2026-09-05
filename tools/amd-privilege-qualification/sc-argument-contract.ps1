#requires -Version 5.1

function New-QualificationServiceCreateArguments {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ServiceName,
        [Parameter(Mandatory = $true)][string]$BinPath,
        [Parameter(Mandatory = $true)][string]$ServiceAccount,
        [Parameter(Mandatory = $true)][string]$DisplayName
    )

    @(
        'create'
        $ServiceName
        'binPath='
        $BinPath
        'start='
        'demand'
        'obj='
        $ServiceAccount
        'type='
        'own'
        'DisplayName='
        $DisplayName
    )
}
