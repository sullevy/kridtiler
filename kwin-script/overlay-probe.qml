import QtQuick
import org.kde.kwin

Item {
    Component.onCompleted: {
        console.warn("[kridtiler-overlay-probe] loaded");
        console.warn("[kridtiler-overlay-probe] activeWindow=" + (Workspace.activeWindow ? Workspace.activeWindow.caption : "null"));
        console.warn("[kridtiler-overlay-probe] activeScreen=" + (Workspace.activeScreen ? Workspace.activeScreen.name : "null"));
    }
}
