using ACadSharp;
using ACadSharp.IO;
using ACadSharp.Entities;

static void Notify(object sender, NotificationEventArgs e)
{
    if (e.NotificationType == NotificationType.Error || e.NotificationType == NotificationType.Warning)
        Console.WriteLine($"  [{e.NotificationType}] {e.Message} {e.Exception?.Message}");
}

var cmd = args[0];
if (cmd == "read")
{
    CadDocument doc;
    if (args[1].EndsWith(".dwg", StringComparison.OrdinalIgnoreCase))
        doc = DwgReader.Read(args[1], new DwgReaderConfiguration { Failsafe = false, CrcCheck = true }, Notify);
    else
        doc = DxfReader.Read(args[1], Notify);
    Console.WriteLine($"version {doc.Header.Version} handseed {doc.Header.HandleSeed:X}");
    var counts = doc.Entities.GroupBy(e => e.ObjectName).OrderBy(g => g.Key).Select(g => $"{g.Key}:{g.Count()}");
    Console.WriteLine("model " + string.Join(" ", counts));
    Console.WriteLine("layers " + string.Join(",", doc.Layers.Select(l => $"{l.Name}/{l.Color}/{l.LineType?.Name}/{l.LineWeight}")));
    Console.WriteLine("ltypes " + string.Join(",", doc.LineTypes.Select(l => l.Name)));
    Console.WriteLine("styles " + string.Join(",", doc.TextStyles.Select(l => l.Name)));
    Console.WriteLine("blocks " + string.Join(",", doc.BlockRecords.Select(b => $"{b.Name}({b.Entities.Count})")));
    Console.WriteLine("layouts " + string.Join(",", doc.Layouts.Select(l => $"{l.Name}->{l.AssociatedBlock?.Name}")));
    foreach (var e in doc.Entities.Take(int.Parse(args.Length > 2 ? args[2] : "0")))
        Console.WriteLine($"  {e.ObjectName} {e.Handle:X} layer={e.Layer?.Name} color={e.Color}");
}
else if (cmd == "classes")
{
    var doc = DwgReader.Read(args[1]);
    foreach (var c in doc.Classes)
        Console.WriteLine($"{c.ClassNumber}|{(int)c.ProxyFlags}|{c.ApplicationName}|{c.CppClassName}|{c.DxfName}|{c.WasZombie}|{c.ItemClassId}");
}
else if (cmd == "new")
{
    var doc = new CadDocument();
    doc.Header.Version = Enum.Parse<ACadVersion>(args[2]);
    doc.Entities.Add(new Line(new CSMath.XYZ(0, 0, 0), new CSMath.XYZ(10, 5, 0)));
    doc.Entities.Add(new Circle { Center = new CSMath.XYZ(5, 5, 0), Radius = 2 });
    doc.Entities.Add(new TextEntity { Value = "Hello", Height = 2.5, InsertPoint = new CSMath.XYZ(1, 1, 0) });
    DwgWriter.Write(args[1], doc, null, Notify);
    Console.WriteLine("written");
}
else if (cmd == "convert")
{
    // convert in.dxf|in.dwg out.dwg|out.dxf [version]
    CadDocument doc = args[1].EndsWith(".dwg", StringComparison.OrdinalIgnoreCase) ? DwgReader.Read(args[1], Notify) : DxfReader.Read(args[1], Notify);
    if (args.Length > 3) doc.Header.Version = Enum.Parse<ACadVersion>(args[3]);
    if (args[2].EndsWith(".dwg", StringComparison.OrdinalIgnoreCase)) DwgWriter.Write(args[2], doc, null, Notify);
    else DxfWriter.Write(args[2], doc, false, null, Notify);
    Console.WriteLine("written");
}
