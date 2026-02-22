import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-shell";
import { initTheme, getResolvedTheme, onThemeChange } from "./theme";

import logoLight from "./assets/flowstt-portrait-light.svg";
import logoDark from "./assets/flowstt-portrait.svg";

const WEBSITE_URL = "https://flowstt.io";
const GITHUB_URL = "https://github.com/flowstt/flowstt";
const LICENSE_URL = "https://github.com/flowstt/flowstt/blob/master/LICENSE";

function isDebugConsoleHotkey(e: KeyboardEvent): boolean {
    const isIKey = e.code === "KeyI" || e.key === "i" || e.key === "I";
    const isCtrlShift = e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;
    const isMetaAlt = e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey;
    return isIKey && (isCtrlShift || isMetaAlt);
}

/**
 * Open an external URL in the default browser.
 */
function openExternal(url: string) {
    void open(url).catch((error) => {
        console.error("Failed to open external link:", error);
    });
}

document.addEventListener("DOMContentLoaded", async () => {
    // Initialize theme before first paint
    await initTheme();

    // Disable default context menu
    document.addEventListener("contextmenu", (e) => {
        e.preventDefault();
    });

    // Suppress all default keyboard behaviour in this decorationless window.
    // See main.ts for detailed explanation of why this is needed.
    const suppressKeyHandler = (e: KeyboardEvent) => {
        if (isDebugConsoleHotkey(e)) return;
        if (e.key === "F4" && e.altKey) return;
        const tag = (e.target as HTMLElement)?.tagName;
        if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") return;
        e.preventDefault();
    };
    document.addEventListener("keydown", suppressKeyHandler);
    document.addEventListener("keyup", suppressKeyHandler);

    // Set version
    try {
        const version = await getVersion();
        const versionEl = document.getElementById("about-version");
        if (versionEl) {
            versionEl.textContent = `Version ${version}`;
        }
    } catch (e) {
        console.error("Failed to get version:", e);
    }

    // Swap logo image based on theme
    const aboutLogo = document.querySelector<HTMLImageElement>(".about-logo");
    if (aboutLogo) {
        const updateLogo = (theme: string) => {
            aboutLogo.src = theme === "light" ? logoLight : logoDark;
        };
        updateLogo(getResolvedTheme());
        onThemeChange(updateLogo);
    }

    // Close button - use destroy() like main window does
    const closeBtn = document.getElementById("close-btn");
    if (closeBtn) {
        closeBtn.addEventListener("click", async (e) => {
            e.preventDefault();
            e.stopPropagation();
            const win = getCurrentWindow();
            await win.destroy();
        });
    }

    // External links
    document.getElementById("link-website")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternal(WEBSITE_URL);
    });

    document.getElementById("link-github")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternal(GITHUB_URL);
    });

    document.getElementById("link-license")?.addEventListener("click", (e) => {
        e.preventDefault();
        openExternal(LICENSE_URL);
    });
});
