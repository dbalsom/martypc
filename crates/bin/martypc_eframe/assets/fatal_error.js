function showMartyFatalError() {
    const errorScreen = document.getElementById("fatal_error_screen");
    if (errorScreen !== null) {
        errorScreen.hidden = false;
    }
}

window.martyShowFatalError = showMartyFatalError;
window.addEventListener("error", showMartyFatalError);
window.addEventListener("unhandledrejection", showMartyFatalError);
