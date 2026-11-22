// Veld .NET global tool wrapper.
// Downloads the pre-built Go binary for the current platform on first run,
// caches it in %LOCALAPPDATA%/veld/{version}/ (Windows) or ~/.local/share/veld/{version}/,
// then proxies all arguments to it.

using System.Diagnostics;
using System.Formats.Tar;
using System.IO.Compression;
using System.Runtime.InteropServices;

const string Version = "0.1.0";
const string Repo    = "Adhamzineldin/Veld";

var binary = await EnsureBinary();
if (binary is null)
{
    await Console.Error.WriteLineAsync("error: veld has no pre-built binary for this platform");
    await Console.Error.WriteLineAsync("       supported: linux/darwin/windows × amd64/arm64");
    return 1;
}

var psi = new ProcessStartInfo(binary) { UseShellExecute = false };
foreach (var a in args) psi.ArgumentList.Add(a);

var proc = Process.Start(psi)!;
proc.WaitForExit();
return proc.ExitCode;

// ── helpers ──────────────────────────────────────────────────────────────────

static string? PlatformKey()
{
    var os = RuntimeInformation.IsOSPlatform(OSPlatform.Windows) ? "windows"
           : RuntimeInformation.IsOSPlatform(OSPlatform.OSX)     ? "darwin"
           : RuntimeInformation.IsOSPlatform(OSPlatform.Linux)   ? "linux"
           : null;
    if (os is null) return null;

    var arch = RuntimeInformation.ProcessArchitecture switch
    {
        Architecture.X64   => "amd64",
        Architecture.Arm64 => "arm64",
        _                  => null,
    };
    if (arch is null) return null;

    return $"{os}-{arch}";
}

static async Task<string?> EnsureBinary()
{
    var key = PlatformKey();
    if (key is null) return null;

    var isWindows  = RuntimeInformation.IsOSPlatform(OSPlatform.Windows);
    var binaryName = isWindows ? "veld.exe" : "veld";

    // Cache at: %LOCALAPPDATA%\veld\{version}\{platform}\veld[.exe]
    var cacheDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "veld", Version, key);
    var binaryPath = Path.Combine(cacheDir, binaryName);

    if (File.Exists(binaryPath)) return binaryPath;

    // Download from GitHub Releases
    Directory.CreateDirectory(cacheDir);
    var ext      = isWindows ? ".zip" : ".tar.gz";
    var archive  = $"veld-{key}{ext}";
    var url      = $"https://github.com/{Repo}/releases/download/v{Version}/{archive}";
    var tmp      = Path.Combine(Path.GetTempPath(), archive);

    await Console.Error.WriteLineAsync($"[veld] downloading binary for {key}...");
    using var http = new HttpClient();
    http.DefaultRequestHeaders.Add("User-Agent", "veld-dotnet-tool");

    try
    {
        var bytes = await http.GetByteArrayAsync(url);
        await File.WriteAllBytesAsync(tmp, bytes);
    }
    catch (Exception ex)
    {
        await Console.Error.WriteLineAsync($"[veld] download failed: {ex.Message}");
        await Console.Error.WriteLineAsync($"       try: go install github.com/{Repo}/cmd/veld@latest");
        return null;
    }

    // Extract
    if (isWindows)
    {
        ZipFile.ExtractToDirectory(tmp, cacheDir, overwriteFiles: true);
    }
    else
    {
        using var gz  = new GZipStream(File.OpenRead(tmp), CompressionMode.Decompress);
        await TarFile.ExtractToDirectoryAsync(gz, cacheDir, overwriteFiles: true);
        File.SetUnixFileMode(binaryPath,
            UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute |
            UnixFileMode.GroupRead | UnixFileMode.GroupExecute |
            UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
    }

    File.Delete(tmp);
    return File.Exists(binaryPath) ? binaryPath : null;
}
