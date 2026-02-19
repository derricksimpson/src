namespace Src.Models;

public sealed class ScanResult
{
    public string Name { get; set; } = string.Empty;
    public List<ScanResult>? Children { get; set; }
    public List<string>? Files { get; set; }
}
