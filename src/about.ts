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
    
    // Secret reset shortcut: Ctrl+Shift+Alt+R
    if (e.ctrlKey && e.shiftKey && e.altKey && e.key === 'R') {
      e.preventDefault();
      
      const confirmed = confirm(
        '⚠️ WARNING: Application Reset ⚠️\n\n' +
        'This will permanently delete:\n' +
        '• All saved credentials\n' +
        '• All RDP connection files\n' +
        '• All saved hosts\n' +
        '• Recent connection history\n\n' +
        'This action CANNOT be undone!\n\n' +
        'Are you sure you want to continue?'
      );
      
      if (!confirmed) {
        return;
      }
      
      try {
        const result = await invoke<string>("reset_application");
        alert(result);
        
        // Recommend restarting the application
        const shouldQuit = confirm(
          'Reset complete!\n\n' +
          'It is recommended to restart the application now.\n\n' +
          'Do you want to quit the application?'
        );
        
        if (shouldQuit) {
          await invoke("quit_app");
        }
      } catch (err) {
        alert('Failed to reset application: ' + err);
        console.error("Reset error:", err);
      }
    }
  });
});
