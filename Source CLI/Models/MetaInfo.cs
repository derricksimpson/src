namespace Src.Models;

public sealed class MetaInfo
{
    public long ElapsedMs { get; set; }
    public bool Timeout { get; set; }
    public int FilesScanned { get; set; }
    public int FilesMatched { get; set; }
}
