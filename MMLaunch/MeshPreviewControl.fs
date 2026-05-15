// Avalonia mesh preview control. Replaces the old WPF PreviewHost which
// embedded a MonoGame D3D11 surface and instantiated MMView's MeshViewControl.
//
// To keep the launcher self-contained and free of the .NET-Framework-only
// MonoGame/SharpDX dependencies that MMView pulls in, this is a small Skia
// based wireframe renderer: it parses the .obj/.mmobj geometry inline,
// projects each triangle through a fixed orbit camera, and draws the edges
// using Avalonia's DrawingContext. Mouse drag rotates, mouse wheel zooms.

namespace MMLaunch

open System
open System.IO
open System.Numerics

open Avalonia
open Avalonia.Controls
open Avalonia.Input
open Avalonia.Media

module private MmObj =
    type Mesh = {
        Positions: Vector3[]
        Triangles: (int * int * int)[]
    }

    let private tryParseFloat (s: string) =
        Single.TryParse(s, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture)

    let private parseVec3 (parts: string[]) =
        if parts.Length < 4 then None
        else
            let ok1, x = tryParseFloat parts.[1]
            let ok2, y = tryParseFloat parts.[2]
            let ok3, z = tryParseFloat parts.[3]
            if ok1 && ok2 && ok3 then Some(Vector3(x, y, z)) else None

    /// Returns the 1-based position index from a face vertex token like
    /// "12", "12/4", "12/4/7", or "12//7".  Negative indices (relative to
    /// end of list) are also handled.
    let private parseFaceIdx (token: string) (positionsCount: int) : int option =
        let posStr =
            match token.IndexOf('/') with
            | -1 -> token
            | i -> token.Substring(0, i)
        match Int32.TryParse(posStr) with
        | true, v when v > 0 -> Some(v - 1)
        | true, v when v < 0 -> Some(positionsCount + v)
        | _ -> None

    let load (path: string) : Mesh =
        let positions = ResizeArray<Vector3>()
        let triangles = ResizeArray<int * int * int>()

        if File.Exists path then
            let lines = File.ReadAllLines path
            for line in lines do
                let trimmed = line.Trim()
                if trimmed.Length > 0 && not (trimmed.StartsWith "#") then
                    let parts = trimmed.Split([| ' '; '\t' |], StringSplitOptions.RemoveEmptyEntries)
                    if parts.Length > 0 then
                        match parts.[0] with
                        | "v" ->
                            match parseVec3 parts with
                            | Some v -> positions.Add(v)
                            | None -> ()
                        | "f" when parts.Length >= 4 ->
                            // Triangulate: fan from vertex 1
                            let idxs =
                                [| for i in 1 .. parts.Length - 1 -> parseFaceIdx parts.[i] positions.Count |]
                            if idxs |> Array.forall Option.isSome then
                                let resolved = idxs |> Array.map Option.get
                                for k in 1 .. resolved.Length - 2 do
                                    triangles.Add((resolved.[0], resolved.[k], resolved.[k + 1]))
                        | _ -> ()

        { Positions = positions.ToArray(); Triangles = triangles.ToArray() }

    let bounds (m: Mesh) : Vector3 * Vector3 =
        if m.Positions.Length = 0 then
            Vector3.Zero, Vector3.One
        else
            let mutable lo = m.Positions.[0]
            let mutable hi = m.Positions.[0]
            for v in m.Positions do
                lo <- Vector3.Min(lo, v)
                hi <- Vector3.Max(hi, v)
            lo, hi

[<AllowNullLiteral>] // FindControl returns null if not present; allow Option round-trip
type MeshPreviewControl() as this =
    inherit Control()

    let mutable mesh: MmObj.Mesh = { Positions = [||]; Triangles = [||] }
    let mutable selectedFile: string = ""

    // Orbit camera state
    let mutable yaw: float32 = 0.5f
    let mutable pitch: float32 = -0.3f
    let mutable distance: float32 = 3.5f
    let mutable center: Vector3 = Vector3.Zero
    let mutable modelScale: float32 = 1.0f

    let mutable lastPointer: Point option = None

    let strokeBrush: IBrush = SolidColorBrush(Color.FromRgb(60uy, 200uy, 255uy)) :> IBrush
    let bgBrush: IBrush = SolidColorBrush(Color.FromRgb(28uy, 28uy, 32uy)) :> IBrush
    let pen = Pen(SolidColorBrush(Color.FromRgb(60uy, 200uy, 255uy)), 1.0)

    let recomputeFraming () =
        let lo, hi = MmObj.bounds mesh
        center <- (lo + hi) * 0.5f
        let extent = hi - lo
        let maxExtent = max (max extent.X extent.Y) extent.Z
        modelScale <- if maxExtent > 0.0001f then 2.0f / maxExtent else 1.0f
        distance <- 3.5f

    do
        this.ClipToBounds <- true
        this.Focusable <- true

    member x.SelectedFile
        with get() = selectedFile
        and set (value: string) =
            selectedFile <- value
            mesh <-
                if String.IsNullOrEmpty value || not (File.Exists value) then
                    { Positions = [||]; Triangles = [||] }
                else
                    MmObj.load value
            recomputeFraming ()
            x.InvalidateVisual()

    override x.OnPointerPressed(e) =
        base.OnPointerPressed(e)
        lastPointer <- Some(e.GetPosition(x))
        e.Pointer.Capture(x) |> ignore

    override x.OnPointerMoved(e) =
        base.OnPointerMoved(e)
        match lastPointer with
        | Some prev when e.GetCurrentPoint(x).Properties.IsLeftButtonPressed ->
            let cur = e.GetPosition(x)
            let dx = float32 (cur.X - prev.X)
            let dy = float32 (cur.Y - prev.Y)
            yaw <- yaw + dx * 0.01f
            pitch <- pitch + dy * 0.01f
            pitch <- max -1.5f (min 1.5f pitch)
            lastPointer <- Some cur
            x.InvalidateVisual()
        | _ -> ()

    override x.OnPointerReleased(e) =
        base.OnPointerReleased(e)
        lastPointer <- None
        e.Pointer.Capture(null) |> ignore

    override x.OnPointerWheelChanged(e) =
        base.OnPointerWheelChanged(e)
        let factor = if e.Delta.Y > 0.0 then 0.9f else 1.1f
        distance <- max 0.5f (min 50.0f (distance * factor))
        x.InvalidateVisual()

    override x.Render(ctx: DrawingContext) =
        let b = x.Bounds
        ctx.FillRectangle(bgBrush, Rect(0.0, 0.0, b.Width, b.Height))

        if mesh.Triangles.Length = 0 then
            let fmt =
                FormattedText(
                    (if String.IsNullOrEmpty selectedFile then "No mesh selected" else "No geometry"),
                    Globalization.CultureInfo.InvariantCulture,
                    FlowDirection.LeftToRight,
                    Typeface.Default,
                    12.0,
                    Brushes.Gray)
            ctx.DrawText(fmt, Point(10.0, 10.0))
        else
            let w = float32 b.Width
            let h = float32 b.Height
            if w > 1.0f && h > 1.0f then
                let cosY = MathF.Cos(yaw)
                let sinY = MathF.Sin(yaw)
                let cosP = MathF.Cos(pitch)
                let sinP = MathF.Sin(pitch)

                // Project a model-space vertex into screen-space.
                let project (v: Vector3) : Point =
                    let p = (v - center) * modelScale
                    // Yaw around Y, then pitch around X.
                    let x1 = p.X * cosY + p.Z * sinY
                    let z1 = -p.X * sinY + p.Z * cosY
                    let y2 = p.Y * cosP - z1 * sinP
                    let z2 = p.Y * sinP + z1 * cosP
                    // Perspective divide; eye at (0, 0, distance) looking toward -Z.
                    let ez = distance - z2
                    let f = 1.5f
                    let sx = if ez > 0.001f then x1 * f / ez else 0.0f
                    let sy = if ez > 0.001f then y2 * f / ez else 0.0f
                    let scale = min w h * 0.45f
                    Point(float (w * 0.5f + sx * scale), float (h * 0.5f - sy * scale))

                let positions = mesh.Positions
                let projected = Array.map project positions

                for (a, ib, c) in mesh.Triangles do
                    if a >= 0 && ib >= 0 && c >= 0
                       && a < projected.Length && ib < projected.Length && c < projected.Length then
                        let pa = projected.[a]
                        let pb = projected.[ib]
                        let pc = projected.[c]
                        ctx.DrawLine(pen, pa, pb)
                        ctx.DrawLine(pen, pb, pc)
                        ctx.DrawLine(pen, pc, pa)
