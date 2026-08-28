const SystemOption = Object.freeze({
    VIDEO_CARD: "video-card",
    MEMORY_SIDECAR: "memory-sidecar",
    ADLIB: "adlib",
});

const HistoryView = Object.freeze({
    LAUNCHER: "launcher",
    EMULATOR: "emulator",
});

const SUPPORTED_SYSTEM_OPTIONS = new Set(Object.values(SystemOption));
const PRIMARY_VIDEO_OVERLAYS = new Set(["ibm_mda", "hercules", "ibm_cga", "ibm_ega", "ibm_vga"]);
const VIDEO_SCALER_PRESETS = new Map([
    ["ibm_mda", "Green CRT"],
    ["hercules", "Amber CRT"],
    ["ibm_cga", "IBM 5153"],
    ["ibm_ega", "IBM 8513"],
    ["ibm_vga", "IBM 8513"],
]);

const MEMORY_SIDECAR_OVERLAY = "pcjr_memory_sidecar";
const ADLIB_OVERLAY = "adlib";
const DEFAULT_VIDEO_OVERLAY = "ibm_cga";

const CAROUSEL_ROTATION_DURATION_MS = 520;
const CAROUSEL_HORIZONTAL_RADIUS_PERCENT = 28;
const CAROUSEL_VERTICAL_RADIUS_PERCENT = 18;
const CAROUSEL_CENTER_TOP_PERCENT = 45;
const SWIPE_THRESHOLD_PX = 42;
const SWIPE_HORIZONTAL_BIAS = 1.15;
const FULL_TURN = Math.PI * 2;

// Capture the classic script URL while document.currentScript is available.
const carouselScriptUrl = new URL(document.currentScript?.src ?? "assets/carousel.js", document.baseURI);
const systemsConfigUrl = new URL("systems.json", carouselScriptUrl);
systemsConfigUrl.search = carouselScriptUrl.search;

const reducedMotionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

let elements = null;

const state = {
    systems: [],
    cards: [],
    wasmReady: false,
    emulatorLaunchStarted: false,
    selectedIndex: 0,
    optionSelections: new Map(),
    carouselPosition: 0,
    carouselTargetPosition: 0,
    carouselAnimationFrame: null,
    pointerStart: null,
    suppressNextClick: false,
    suppressClickResetTimer: null,
};

function requiredElement(id) {
    const element = document.getElementById(id);
    if (element === null) {
        throw new Error(`Required carousel element #${id} was not found.`);
    }

    return element;
}

function collectElements() {
    return {
        carousel: requiredElement("system_carousel"),
        startScreen: requiredElement("start_screen"),
        previousSystemButton: requiredElement("previous_system"),
        nextSystemButton: requiredElement("next_system"),
        selectedSystemName: requiredElement("selected_system_name"),
        selectedSystemDescription: requiredElement("selected_system_description"),
        systemDetailsPanel: requiredElement("system_details_panel"),
        systemOptions: requiredElement("system_options"),
        videoCardOption: requiredElement("video_card_option"),
        videoCardSelect: requiredElement("video_card_select"),
        memorySidecarOption: requiredElement("memory_sidecar_option"),
        memorySidecarCheckbox: requiredElement("memory_sidecar_checkbox"),
        adlibOption: requiredElement("adlib_option"),
        adlibCheckbox: requiredElement("adlib_checkbox"),
        startButton: requiredElement("start_logo"),
    };
}

function configureTouchUi() {
    if ("ontouchstart" in window || navigator.maxTouchPoints > 0) {
        document.documentElement.classList.add("touch-device");
    }
}

function prefersReducedMotion() {
    return reducedMotionQuery.matches;
}

function validateSystems(value) {
    if (!Array.isArray(value) || value.length === 0) {
        throw new Error("System configuration must contain at least one system.");
    }

    const stringFields = ["id", "machineConfig", "configFamily", "name", "description", "image"];
    const uniqueFields = ["id", "machineConfig", "configFamily"];
    const seenValues = new Map(uniqueFields.map((field) => [field, new Set()]));

    return value.map((system, index) => {
        if (system === null || typeof system !== "object" || Array.isArray(system)) {
            throw new Error(`System ${index} must be an object.`);
        }

        for (const field of stringFields) {
            if (typeof system[field] !== "string" || system[field].trim().length === 0) {
                throw new Error(`System ${index} has an invalid ${field}.`);
            }
        }

        for (const field of uniqueFields) {
            const values = seenValues.get(field);
            if (values.has(system[field])) {
                throw new Error(`System ${index} has a duplicate ${field}: ${system[field]}.`);
            }
            values.add(system[field]);
        }

        if (!Array.isArray(system.options) || !system.options.every((option) => typeof option === "string")) {
            throw new Error(`System ${index} has an invalid options list.`);
        }

        const options = new Set(system.options);
        if (options.size !== system.options.length) {
            throw new Error(`System ${index} has duplicate options.`);
        }

        for (const option of options) {
            if (!SUPPORTED_SYSTEM_OPTIONS.has(option)) {
                throw new Error(`System ${index} has an unknown option: ${option}.`);
            }
        }

        return {...system, options};
    });
}

async function loadSystems() {
    const response = await fetch(systemsConfigUrl);
    if (!response.ok) {
        throw new Error(`Failed to load ${systemsConfigUrl}: HTTP ${response.status}`);
    }

    return validateSystems(await response.json());
}

function systemSupportsOption(system, option) {
    return system.options.has(option);
}

function activeSystem() {
    return state.systems[state.selectedIndex];
}

function activeOptionSelection() {
    return state.optionSelections.get(activeSystem().id);
}

function createSystemCard(system, index) {
    const card = document.createElement("button");
    card.className = "system-card";
    card.type = "button";
    card.dataset.position = index === 0 ? "active" : "orbit";
    card.setAttribute("aria-label", `Select ${system.name}`);
    card.setAttribute("aria-pressed", (index === 0).toString());

    const image = document.createElement("img");
    image.src = system.image;
    image.alt = system.name;
    card.append(image);

    return card;
}

function readConfiguredOverlays(url = new URL(window.location.href)) {
    const parameter = url.searchParams.get("machine_config_overlays");
    return parameter?.split(",").map((overlay) => overlay.trim()).filter(Boolean) ?? [];
}

function primaryVideoOverlayName(overlay) {
    const [name, parameter] = overlay.split(":", 2);
    if (!PRIMARY_VIDEO_OVERLAYS.has(name)) {
        return null;
    }

    if ((name === "ibm_mda" || name === "hercules") && parameter !== undefined && parameter !== "0") {
        return null;
    }

    return name;
}

function findPrimaryVideoOverlay(overlays) {
    for (let index = overlays.length - 1; index >= 0; index -= 1) {
        const videoOverlay = primaryVideoOverlayName(overlays[index]);
        if (videoOverlay !== null) {
            return videoOverlay;
        }
    }

    return null;
}

function readLaunchConfiguration() {
    const url = new URL(window.location.href);
    const overlays = readConfiguredOverlays(url);
    return {
        machineConfig: url.searchParams.get("machine_config_name"),
        overlays,
        videoOverlay: findPrimaryVideoOverlay(overlays),
    };
}

function findConfiguredSystemIndex(machineConfig) {
    if (machineConfig === null) {
        return -1;
    }

    const exactIndex = state.systems.findIndex((system) => system.machineConfig === machineConfig);
    if (exactIndex >= 0) {
        return exactIndex;
    }

    return state.systems.findIndex((system) => machineConfig.startsWith(`${system.configFamily}_`));
}

function initializeOptionSelections(launchConfiguration, configuredIndex) {
    state.optionSelections.clear();

    for (const system of state.systems) {
        state.optionSelections.set(system.id, {
            videoOverlay: systemSupportsOption(system, SystemOption.VIDEO_CARD) ? DEFAULT_VIDEO_OVERLAY : null,
            memorySidecar: systemSupportsOption(system, SystemOption.MEMORY_SIDECAR),
            adlib: false,
        });
    }

    const system = activeSystem();
    const selection = activeOptionSelection();

    if (systemSupportsOption(system, SystemOption.VIDEO_CARD) && launchConfiguration.videoOverlay !== null) {
        selection.videoOverlay = launchConfiguration.videoOverlay;
    }

    if (configuredIndex >= 0 && systemSupportsOption(system, SystemOption.MEMORY_SIDECAR)) {
        selection.memorySidecar = launchConfiguration.overlays.includes(MEMORY_SIDECAR_OVERLAY);
    }

    if (configuredIndex >= 0 && systemSupportsOption(system, SystemOption.ADLIB)) {
        selection.adlib = launchConfiguration.overlays.includes(ADLIB_OVERLAY);
    }
}

function syncLaunchUrl() {
    const system = activeSystem();
    const selection = activeOptionSelection();
    const videoOverlay = systemSupportsOption(system, SystemOption.VIDEO_CARD) ? selection.videoOverlay : null;
    const includeMemorySidecar = systemSupportsOption(system, SystemOption.MEMORY_SIDECAR) && selection.memorySidecar;
    const includeAdlib = systemSupportsOption(system, SystemOption.ADLIB) && selection.adlib;
    const url = new URL(window.location.href);
    const overlays = readConfiguredOverlays(url).filter((overlay) =>
        primaryVideoOverlayName(overlay) === null &&
        overlay !== MEMORY_SIDECAR_OVERLAY &&
        overlay !== ADLIB_OVERLAY
    );

    if (videoOverlay !== null) {
        // Primary video overlays replace the adapter list, so they must run
        // before any preserved secondary-card merge such as ibm_mda:1.
        overlays.unshift(videoOverlay);
    }
    if (includeMemorySidecar) {
        overlays.push(MEMORY_SIDECAR_OVERLAY);
    }
    if (includeAdlib) {
        overlays.push(ADLIB_OVERLAY);
    }

    url.searchParams.set("machine_config_name", system.machineConfig);

    if (overlays.length > 0) {
        url.searchParams.set("machine_config_overlays", overlays.join(","));
    } else {
        url.searchParams.delete("machine_config_overlays");
    }

    const scalerPreset = VIDEO_SCALER_PRESETS.get(videoOverlay);
    if (scalerPreset !== undefined) {
        url.searchParams.set("scaler_preset", scalerPreset);
    } else {
        url.searchParams.delete("scaler_preset");
    }

    window.history.replaceState({martyView: HistoryView.LAUNCHER}, "", url.toString());
}

function setOptionVisibility(container, control, visible) {
    container.dataset.visible = visible.toString();
    container.setAttribute("aria-hidden", (!visible).toString());
    control.disabled = !visible;
}

function renderSystemOptions() {
    const system = activeSystem();
    const selection = activeOptionSelection();
    const supportsVideoCard = systemSupportsOption(system, SystemOption.VIDEO_CARD);
    const supportsMemorySidecar = systemSupportsOption(system, SystemOption.MEMORY_SIDECAR);
    const supportsAdlib = systemSupportsOption(system, SystemOption.ADLIB);
    const hasOptions = supportsVideoCard || supportsMemorySidecar || supportsAdlib;

    elements.systemOptions.dataset.visible = hasOptions.toString();
    elements.systemOptions.setAttribute("aria-hidden", (!hasOptions).toString());
    setOptionVisibility(elements.videoCardOption, elements.videoCardSelect, supportsVideoCard);
    setOptionVisibility(elements.memorySidecarOption, elements.memorySidecarCheckbox, supportsMemorySidecar);
    setOptionVisibility(elements.adlibOption, elements.adlibCheckbox, supportsAdlib);

    elements.videoCardSelect.value = selection.videoOverlay ?? DEFAULT_VIDEO_OVERLAY;
    elements.memorySidecarCheckbox.checked = selection.memorySidecar;
    elements.adlibCheckbox.checked = selection.adlib;
}

function renderSystemDetails() {
    const system = activeSystem();
    elements.selectedSystemName.textContent = system.name;
    elements.selectedSystemDescription.textContent = system.description;
    renderSystemOptions();
}

function renderStartButton() {
    elements.startButton.textContent = `Start ${activeSystem().name}`;
}

function wrapSystemIndex(index) {
    const systemCount = state.systems.length;
    return ((index % systemCount) + systemCount) % systemCount;
}

function renderCarousel(settled = false) {
    state.cards.forEach((card, index) => {
        const angle = (index - state.carouselPosition) * FULL_TURN / state.cards.length;
        const horizontalPosition = Math.sin(angle);
        const depth = Math.cos(angle);
        const normalizedDepth = (depth + 1) / 2;
        const isSelected = index === state.selectedIndex;
        const isActive = settled && isSelected;

        card.dataset.position = isActive ? "active" : "orbit";
        card.style.setProperty("--carousel-left", `${50 + horizontalPosition * CAROUSEL_HORIZONTAL_RADIUS_PERCENT}%`);
        card.style.setProperty(
            "--carousel-top",
            `${CAROUSEL_CENTER_TOP_PERCENT + depth * CAROUSEL_VERTICAL_RADIUS_PERCENT}%`,
        );
        card.style.setProperty("--carousel-scale", (0.75 + depth * 0.25).toFixed(3));
        card.style.setProperty("--carousel-rotation", `${horizontalPosition * -28}deg`);
        card.style.setProperty("--carousel-brightness", (0.28 + normalizedDepth * 0.84).toFixed(3));
        card.style.setProperty("--carousel-saturation", (0.42 + normalizedDepth * 0.64).toFixed(3));
        card.style.setProperty("--carousel-opacity", (0.45 + normalizedDepth * 0.55).toFixed(3));
        card.style.setProperty("--carousel-z", Math.round(normalizedDepth * 100 + 1));
        card.setAttribute("aria-pressed", isSelected.toString());
        card.tabIndex = isSelected ? 0 : -1;
    });
}

function easeInOutCubic(position) {
    return position < 0.5
        ? 4 * position * position * position
        : 1 - Math.pow(-2 * position + 2, 3) / 2;
}

function animateCarousel(targetPosition) {
    if (state.carouselAnimationFrame !== null) {
        cancelAnimationFrame(state.carouselAnimationFrame);
        state.carouselAnimationFrame = null;
    }

    const startPosition = state.carouselPosition;
    const distance = Math.abs(targetPosition - startPosition);

    if (prefersReducedMotion() || distance === 0) {
        state.carouselPosition = targetPosition;
        renderCarousel(true);
        return;
    }

    const duration = CAROUSEL_ROTATION_DURATION_MS * Math.sqrt(distance);
    let startTime = null;
    renderCarousel(false);

    function animateFrame(timestamp) {
        if (startTime === null) {
            startTime = timestamp;
        }

        const progress = Math.min((timestamp - startTime) / duration, 1);
        state.carouselPosition = startPosition + (targetPosition - startPosition) * easeInOutCubic(progress);
        renderCarousel(false);

        if (progress < 1) {
            state.carouselAnimationFrame = requestAnimationFrame(animateFrame);
        } else {
            state.carouselPosition = targetPosition;
            state.carouselAnimationFrame = null;
            renderCarousel(true);
        }
    }

    state.carouselAnimationFrame = requestAnimationFrame(animateFrame);
}

function shortestRotationTo(index) {
    const forward = wrapSystemIndex(index - state.selectedIndex);
    return forward <= state.cards.length / 2 ? forward : forward - state.cards.length;
}

function completeSystemDetailsTransition() {
    renderSystemDetails();
    elements.systemDetailsPanel.classList.remove("is-changing");
}

function transitionSystemDetails(selectionChanged) {
    if (prefersReducedMotion() || !selectionChanged) {
        elements.systemDetailsPanel.classList.remove("is-changing");
        renderSystemDetails();
        return;
    }

    elements.systemDetailsPanel.classList.add("is-changing");
}

function selectSystem(index, focusCard = false, rotation = null) {
    if (state.systems.length === 0 || state.carouselAnimationFrame !== null) {
        return;
    }

    const nextIndex = wrapSystemIndex(index);
    const rotationAmount = rotation ?? shortestRotationTo(nextIndex);
    const selectionChanged = nextIndex !== state.selectedIndex;

    state.selectedIndex = nextIndex;
    state.carouselTargetPosition += rotationAmount;

    // Commit launch state immediately. The details panel can finish its
    // presentation transition without leaving the Start button or URL stale.
    syncLaunchUrl();
    renderStartButton();
    transitionSystemDetails(selectionChanged);
    animateCarousel(state.carouselTargetPosition);

    if (focusCard) {
        state.cards[state.selectedIndex].focus();
    }
}

function rotateCarousel(direction, focusCard = false) {
    selectSystem(state.selectedIndex + direction, focusCard, direction);
}

function handleSystemDetailsTransitionEnd(event) {
    if (
        event.target === elements.systemDetailsPanel &&
        event.propertyName === "opacity" &&
        elements.systemDetailsPanel.classList.contains("is-changing")
    ) {
        completeSystemDetailsTransition();
    }
}

function handleReducedMotionChange(event) {
    if (event.matches && elements.systemDetailsPanel.classList.contains("is-changing")) {
        completeSystemDetailsTransition();
    }
}

function handleVideoCardChange() {
    const system = activeSystem();
    if (!systemSupportsOption(system, SystemOption.VIDEO_CARD)) {
        return;
    }

    activeOptionSelection().videoOverlay = elements.videoCardSelect.value;
    syncLaunchUrl();
}

function handleMemorySidecarChange() {
    const system = activeSystem();
    if (!systemSupportsOption(system, SystemOption.MEMORY_SIDECAR)) {
        return;
    }

    activeOptionSelection().memorySidecar = elements.memorySidecarCheckbox.checked;
    syncLaunchUrl();
}

function handleAdlibChange() {
    const system = activeSystem();
    if (!systemSupportsOption(system, SystemOption.ADLIB)) {
        return;
    }

    activeOptionSelection().adlib = elements.adlibCheckbox.checked;
    syncLaunchUrl();
}

function handleCarouselClick(event) {
    if (state.suppressNextClick) {
        state.suppressNextClick = false;
        event.preventDefault();
        return;
    }

    const card = event.target.closest?.(".system-card");
    const cardIndex = state.cards.indexOf(card);
    if (cardIndex >= 0) {
        selectSystem(cardIndex);
    }
}

function handleKeyDown(event) {
    const isFormControl = event.target.closest?.("select, input, textarea, [contenteditable='true']");
    if (
        state.systems.length === 0 ||
        elements.startScreen.getClientRects().length === 0 ||
        isFormControl ||
        event.ctrlKey ||
        event.altKey ||
        event.metaKey
    ) {
        return;
    }

    const isCarouselNavigationKey = ["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key);
    if (!isCarouselNavigationKey) {
        return;
    }

    event.preventDefault();
    if (event.repeat) {
        return;
    }

    if (event.key === "ArrowLeft") {
        rotateCarousel(-1, true);
    } else if (event.key === "ArrowRight") {
        rotateCarousel(1, true);
    } else if (event.key === "Home") {
        selectSystem(0, true);
    } else if (event.key === "End") {
        selectSystem(state.cards.length - 1, true);
    }
}

function handlePointerDown(event) {
    if (!event.isPrimary || (event.pointerType === "mouse" && event.button !== 0)) {
        return;
    }

    state.pointerStart = {id: event.pointerId, x: event.clientX, y: event.clientY};
    elements.carousel.setPointerCapture?.(event.pointerId);
}

function scheduleGeneratedClickSuppression() {
    state.suppressNextClick = true;
    if (state.suppressClickResetTimer !== null) {
        window.clearTimeout(state.suppressClickResetTimer);
    }

    state.suppressClickResetTimer = window.setTimeout(() => {
        state.suppressNextClick = false;
        state.suppressClickResetTimer = null;
    }, 0);
}

function handlePointerUp(event) {
    if (state.pointerStart === null || state.pointerStart.id !== event.pointerId) {
        return;
    }

    const deltaX = event.clientX - state.pointerStart.x;
    const deltaY = event.clientY - state.pointerStart.y;
    state.pointerStart = null;

    if (Math.abs(deltaX) >= SWIPE_THRESHOLD_PX && Math.abs(deltaX) > Math.abs(deltaY) * SWIPE_HORIZONTAL_BIAS) {
        scheduleGeneratedClickSuppression();
        rotateCarousel(deltaX < 0 ? 1 : -1);
    }
}

function handlePointerCancellation(event) {
    if (state.pointerStart?.id === event.pointerId) {
        state.pointerStart = null;
    }
}

function handleStartButtonClick() {
    if (state.emulatorLaunchStarted) {
        return;
    }

    state.emulatorLaunchStarted = true;
    // Establish the Back destination before asynchronous emulator initialization begins.
    window.history.pushState({martyView: HistoryView.EMULATOR}, "", window.location.href);
    elements.startButton.disabled = true;
    elements.startButton.textContent = "Loading…";
    elements.startButton.setAttribute("aria-busy", "true");
}

function reloadLauncher() {
    // Reloading tears down the entire WASM instance and restores the launcher's original DOM.
    window.location.reload();
}

function returnToLauncher() {
    if (window.history.state?.martyView === HistoryView.EMULATOR) {
        window.history.back();
    } else {
        reloadLauncher();
    }
}

function handleHistoryNavigation(event) {
    if (state.emulatorLaunchStarted && event.state?.martyView === HistoryView.LAUNCHER) {
        reloadLauncher();
    }
}

window.martyReturnToLauncher = returnToLauncher;

function updateStartButtonAvailability() {
    elements.startButton.disabled = state.systems.length === 0 || !state.wasmReady;
}

function handleWasmReady() {
    state.wasmReady = true;
    updateStartButtonAvailability();
}

function bindEventListeners() {
    elements.previousSystemButton.addEventListener("click", () => rotateCarousel(-1));
    elements.nextSystemButton.addEventListener("click", () => rotateCarousel(1));
    elements.videoCardSelect.addEventListener("change", handleVideoCardChange);
    elements.memorySidecarCheckbox.addEventListener("change", handleMemorySidecarChange);
    elements.adlibCheckbox.addEventListener("change", handleAdlibChange);
    elements.carousel.addEventListener("click", handleCarouselClick);
    elements.carousel.addEventListener("pointerdown", handlePointerDown);
    elements.carousel.addEventListener("pointerup", handlePointerUp);
    elements.carousel.addEventListener("pointercancel", handlePointerCancellation);
    elements.carousel.addEventListener("lostpointercapture", handlePointerCancellation);
    elements.systemDetailsPanel.addEventListener("transitionend", handleSystemDetailsTransitionEnd);
    elements.startButton.addEventListener("click", handleStartButtonClick);
    window.addEventListener("popstate", handleHistoryNavigation);
    document.addEventListener("keydown", handleKeyDown);
    reducedMotionQuery.addEventListener("change", handleReducedMotionChange);
}

async function initializeCarousel() {
    elements = collectElements();
    configureTouchUi();
    window.history.replaceState({martyView: HistoryView.LAUNCHER}, "", window.location.href);
    state.wasmReady = elements.startButton.dataset.wasmReady === "true";
    document.addEventListener("marty-wasm-ready", handleWasmReady, {once: true});

    const launchConfiguration = readLaunchConfiguration();
    state.systems = await loadSystems();
    state.cards = state.systems.map(createSystemCard);
    elements.carousel.replaceChildren(...state.cards);

    const configuredIndex = findConfiguredSystemIndex(launchConfiguration.machineConfig);
    state.selectedIndex = configuredIndex >= 0 ? configuredIndex : 0;
    state.carouselPosition = state.selectedIndex;
    state.carouselTargetPosition = state.selectedIndex;
    initializeOptionSelections(launchConfiguration, configuredIndex);
    bindEventListeners();

    renderStartButton();
    renderSystemDetails();
    renderCarousel(true);

    // Preserve recognized, explicitly requested profiles. Otherwise make the
    // carousel's initial selection and launch URL agree before enabling Start.
    if (configuredIndex < 0) {
        syncLaunchUrl();
    }

    const hasMultipleSystems = state.systems.length > 1;
    elements.previousSystemButton.disabled = !hasMultipleSystems;
    elements.nextSystemButton.disabled = !hasMultipleSystems;
    updateStartButtonAvailability();
}

initializeCarousel().catch((error) => {
    console.error("Failed to initialize the system carousel:", error);
    window.martyShowFatalError?.();
});
