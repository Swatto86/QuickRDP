import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

async function initializeTheme() {
  let defaultTheme = 'dark';
  
  // Try to get saved theme preference
  try {
    defaultTheme = await invoke<string>('get_theme');
  } catch (error) {
    console.log('Could not get saved theme, using dark as default:', error);
  }
  
  document.documentElement.setAttribute('data-theme', defaultTheme);
  
  // Listen for theme change events
  await listen<string>('theme-changed', (event) => {
    const newTheme = event.payload;
    document.documentElement.setAttribute('data-theme', newTheme);
    console.log('Theme changed to:', newTheme);
  });
}

document.addEventListener("DOMContentLoaded", async () => {
  await initializeTheme();
  
  const closeBtn = document.getElementById("closeBtn");
  if (closeBtn) {
    closeBtn.addEventListener("click", async () => {
      const window = getCurrentWindow();
      await window.hide();
    });
  }

  // Close on Escape key
  window.addEventListener("keydown", async (e) => {
    if (e.key === "Escape") {
      const window = getCurrentWindow();
      await window.hide();
    }
  });
});
