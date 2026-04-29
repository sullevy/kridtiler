// Smoke test: move active window to left half of its monitor's MaximizeArea.
// Loaded via: qdbus org.kde.KWin /Scripting loadScript <path> kridtiler-smoke
//             qdbus org.kde.KWin /Scripting start
//             qdbus org.kde.KWin /Scripting unloadScript kridtiler-smoke

(function () {
    print("[kridtiler-smoke] start");
    const w = workspace.activeWindow;
    if (!w) { print("[kridtiler-smoke] no active window"); return; }
    print("[kridtiler-smoke] target: " + w.resourceClass + " / " + w.caption);

    if (w.fullScreen) w.fullScreen = false;

    const area = workspace.clientArea(KWin.MaximizeArea, w.output, workspace.currentDesktop);
    print("[kridtiler-smoke] area: " + area.x + "," + area.y + " " + area.width + "x" + area.height);

    const target = {
        x: area.x,
        y: area.y,
        width: Math.floor(area.width / 2),
        height: area.height
    };
    w.frameGeometry = target;
    const got = w.frameGeometry;
    print("[kridtiler-smoke] requested " + JSON.stringify(target));
    print("[kridtiler-smoke] result    " + got.x + "," + got.y + " " + got.width + "x" + got.height);

    // Visible side effect so we know the script ran even without journal access:
    // briefly mark the window keep-above. Toggle off after 1.5s via a follow-up call.
    w.keepAbove = true;
})();
