namespace Src.Models;

public sealed class OutputEnvelope
{
    public MetaInfo? Meta { get; set; }
    public List<FileEntry>? Files { get; set; }
    public ScanResult? Tree { get; set; }
    public string? Error { get; set; }
}
