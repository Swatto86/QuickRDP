import { invoke } from "@tauri-apps/api/core";

interface StoredCredentials {
  username: string;
  password: string;
}

interface Host {
    hostname: string;
    description: string;
}

function showNotification(message: string, isError: boolean = false) {
  const notification = document.createElement("div");
  notification.className = `
        fixed bottom-2 left-1/2 transform -translate-x-1/2
        ${isError ? "bg-red-500" : "bg-green-500"}
        text-white px-4 py-2 rounded-md shadow-lg
        text-center min-w-[200px] whitespace-nowrap
        text-sm
    `;
  notification.textContent = message;
  document.body.appendChild(notification);

  setTimeout(() => {
    notification.remove();
  }, 1000);
}

// Function to update button states
function updateButtonStates(hasCredentials: boolean) {
  const deleteBtn = document.querySelector(
    "#delete-btn"
  ) as HTMLButtonElement | null;
  if (deleteBtn) {
    deleteBtn.disabled = !hasCredentials;
    deleteBtn.classList.toggle("opacity-50", !hasCredentials);
    deleteBtn.classList.toggle("cursor-not-allowed", !hasCredentials);
  }
}

// Function to validate form
function validateForm() {
  const okBtn = document.querySelector(
    'button[type="submit"]'
  ) as HTMLButtonElement | null;
  const username = document.querySelector(
    "#username"
  ) as HTMLInputElement | null;
  const password = document.querySelector(
    "#password"
  ) as HTMLInputElement | null;

  if (okBtn && username && password) {
    const isValid =
      username.value.trim() !== "" && password.value.trim() !== "";
    okBtn.disabled = !isValid;
    okBtn.classList.toggle("opacity-50", !isValid);
    okBtn.classList.toggle("cursor-not-allowed", !isValid);
  }
}

// Function to cancel auto-close timer
function cancelAutoCloseTimer() {
  if (autoCloseTimer !== null) {
    clearTimeout(autoCloseTimer);
    autoCloseTimer = null;
  }
  if (countdownInterval !== null) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
  if (animationFrameId !== null) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  // Hide the timer notification
  if (timerNotificationElement) {
    timerNotificationElement.classList.add("hidden");
  }
}

// Check credentials existence
async function checkCredentialsExist() {
  try {
    const stored = await invoke<StoredCredentials>("get_stored_credentials");
    updateButtonStates(!!stored);

    // If credentials exist, populate the form
    if (stored) {
      const username = document.querySelector(
        "#username"
      ) as HTMLInputElement | null;
      const password = document.querySelector(
        "#password"
      ) as HTMLInputElement | null;
      
      if (username && password) {
        username.value = stored.username;
        password.value = stored.password;
        validateForm();
        
        // Only start the auto-close timer if NOT an intentional return
        if (!isIntentionalReturn) {
          // Start the auto-close timer
          timerNotificationElement = document.querySelector("#timer-notification");
          countdownElement = document.querySelector("#countdown");
          
          if (timerNotificationElement && countdownElement) {
            remainingSeconds = 5;
            countdownElement.textContent = String(remainingSeconds);
            
            timerNotificationElement.classList.remove("hidden");
            timerNotificationElement.style.display = 'block';
            timerNotificationElement.style.visibility = 'visible';
            timerNotificationElement.style.opacity = '1';
            
            let lastUpdate = Date.now();
            const loop = function() {
              const now = Date.now();
              if (now - lastUpdate >= 1000) {
                lastUpdate = now;
                remainingSeconds--;
                
                if (countdownElement) countdownElement.textContent = String(remainingSeconds);
                if (timerNotificationElement) {
                  timerNotificationElement.style.backgroundColor = 
                    remainingSeconds % 2 === 0 ? 'rgba(59, 130, 246, 0.3)' : 'rgba(59, 130, 246, 0.2)';
                }
                
                if (remainingSeconds <= 0) {
                  // Hide the banner before switching windows
                  if (timerNotificationElement) {
                    timerNotificationElement.classList.add("hidden");
                    timerNotificationElement.style.display = '';
                    timerNotificationElement.style.visibility = '';
                    timerNotificationElement.style.opacity = '';
                  }
                  // Just hide login window but set last hidden to "main" so tray shows main window
                  invoke("close_login_and_prepare_main");
                  return;
                }
              }
              requestAnimationFrame(loop);
            };
            requestAnimationFrame(loop);
          }
        } else {
          // Reset the flag after use
          isIntentionalReturn = false;
        }
      }
    }
  } catch (err) {
    console.error("Error checking credentials:", err);
    updateButtonStates(false);
  }
}

// Declare the function as a global
declare global {
  interface Window {
    toggleTheme: (themeName: string) => void;
  }
}

// Assign the function to window object
window.toggleTheme = function(themeName: string) {
  document.documentElement.setAttribute('data-theme', themeName);
  localStorage.setItem('theme', themeName);
};

async function initializeTheme() {
  let defaultTheme = 'dark';
  
  // Try to get Windows theme preference
  try {
    defaultTheme = await invoke<string>('get_windows_theme');
  } catch (error) {
    console.log('Could not get Windows theme, using dark as default:', error);
  }
  
  const savedTheme = localStorage.getItem('theme') || defaultTheme;
  document.documentElement.setAttribute('data-theme', savedTheme);
  
  // Add click handlers for theme menu items
  document.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    const themeValue = target.getAttribute('data-theme-value');
    
    if (themeValue) {
      document.documentElement.setAttribute('data-theme', themeValue);
      localStorage.setItem('theme', themeValue);
      
      // Find and close the dropdown by removing focus
      const dropdownContent = target.closest('.dropdown-content');
      if (dropdownContent) {
        (dropdownContent as HTMLElement).blur();
        // Also blur the parent dropdown
        const dropdown = dropdownContent.parentElement;
        if (dropdown) {
          dropdown.blur();
        }
      }
    }
  });
}

// Declare this once at the top of the file
let searchTimeout: ReturnType<typeof setTimeout>;
let autoCloseTimer: ReturnType<typeof setTimeout> | null = null;
let countdownInterval: ReturnType<typeof setInterval> | null = null;
let remainingSeconds = 5;
let animationFrameId: number | null = null;

// Store DOM element references globally to avoid re-querying
let countdownElement: HTMLElement | null = null;
let timerNotificationElement: HTMLElement | null = null;

// Flag to track if user intentionally returned to login (don't show timer)
let isIntentionalReturn = false;

async function handleSearch() {
    const searchInput = document.querySelector("#search-input") as HTMLInputElement;
    const serverList = document.querySelector("#server-list") as HTMLElement;
    
    if (!searchInput || !serverList) return;

    try {
        // If search input is empty, clear the list and show default message
        if (!searchInput.value.trim()) {
            serverList.innerHTML = `
                <div class="text-center text-base-content/60 p-4">
                    Search for servers to connect
                </div>
            `;
            return;
        }

        const results = await invoke<Host[]>("search_hosts", {
            query: searchInput.value
        });

        // Clear existing items
        serverList.innerHTML = "";

        if (results.length === 0) {
            serverList.innerHTML = `
                <div class="text-center text-base-content/60 p-4">
                    No matching hosts found
                </div>
            `;
            return;
        }

        // Add new results
        results.forEach(host => {
            const item = document.createElement("div");
            item.className = "flex items-center justify-between p-4 border-b border-base-300 last:border-b-0";
            
            item.innerHTML = `
                <div class="flex flex-col">
                    <span class="font-medium">${host.hostname}</span>
                    <span class="text-sm opacity-70">${host.description}</span>
                </div>
                <button class="connect-btn btn btn-primary btn-sm">
                    Connect
                </button>
            `;

            // Add click handler for the connect button
            const connectBtn = item.querySelector('.connect-btn');
            if (connectBtn) {
                connectBtn.addEventListener('click', async (e) => {
                    e.stopPropagation();
                    try {
                        await invoke("launch_rdp", { host });
                    } catch (err) {
                        console.error("Failed to connect:", err);
                    }
                });
            }

            serverList.appendChild(item);
        });
    } catch (err) {
        console.error("Search failed:", err);
        serverList.innerHTML = `
            <div class="text-center text-error p-4">
                Failed to search hosts
            </div>
        `;
    }
}

function initializeSearch() {
    const searchInput = document.querySelector("#search-input") as HTMLInputElement;
    
    if (searchInput) {
        // Handle input changes with debounce
        searchInput.addEventListener("input", () => {
            clearTimeout(searchTimeout);
            searchTimeout = setTimeout(() => {
                handleSearch();
            }, 300);
        });
    }
}

function initializeServerList() {
    const serverList = document.querySelector("#server-list") as HTMLElement;
    if (serverList) {
        serverList.innerHTML = `
            <div class="text-center text-base-content/60 p-4">
                Search for servers to connect
            </div>
        `;
    }
}

// Function to hide timer notification
function hideTimerNotification() {
    const timerNotif = document.querySelector("#timer-notification") as HTMLElement | null;
    if (timerNotif) {
      // Clear inline styles that override the hidden class
      timerNotif.classList.add("hidden");
      timerNotif.style.display = '';
      timerNotif.style.visibility = '';
      timerNotif.style.opacity = '';
    }
}

// Modify the main DOMContentLoaded event listener
document.addEventListener("DOMContentLoaded", async () => {
    await initializeTheme();
    initializeSearch();
    initializeServerList();
    
    // Banner is already hidden in HTML with class="hidden", no need to hide on page load
    // It will be shown only when auto-close timer starts (if credentials exist)
    
    // Get form elements
    const form = document.querySelector("#login-form") as HTMLFormElement | null;
    const username = document.querySelector(
        "#username"
    ) as HTMLInputElement | null;
    const password = document.querySelector(
        "#password"
    ) as HTMLInputElement | null;
    const deleteBtn = document.querySelector(
        "#delete-btn"
    ) as HTMLButtonElement | null;
    const cancelBtn = document.querySelector(
        "#cancel-btn"
    ) as HTMLButtonElement | null;
    const okBtn = document.querySelector(
        'button[type="submit"]'
    ) as HTMLButtonElement | null;

    // LOGIN-SPECIFIC CODE - Only run if we're on the login page
    if (form) {
        // Set initial button states
        if (okBtn) {
            okBtn.disabled = true;
            okBtn.classList.add("opacity-50", "cursor-not-allowed");
        }
        if (deleteBtn) {
            deleteBtn.disabled = true;
            deleteBtn.classList.add("opacity-50", "cursor-not-allowed");
        }

        // Check for existing credentials FIRST (before adding event listeners that might cancel the timer)
        checkCredentialsExist();

        // Add input listeners AFTER checking credentials to prevent accidental cancellation
        // Only hide banner on actual typing (input event), not on focus
        if (username) {
            username.addEventListener("input", () => {
                validateForm();
                hideTimerNotification();
                cancelAutoCloseTimer(); // Cancel timer when user types
            });
        }
        if (password) {
            password.addEventListener("input", () => {
                validateForm();
                hideTimerNotification();
                cancelAutoCloseTimer(); // Cancel timer when user types
            });
        }
        
        // Set initial focus AFTER setting up event listeners
        // But DON'T focus if we just showed the auto-close timer (let user see the countdown)
        // Focus will be set when user clicks
        // Removed: if (username) { username.focus(); }

        // Handle delete
        if (deleteBtn) {
            deleteBtn.addEventListener("click", async () => {
                hideTimerNotification();
                cancelAutoCloseTimer();
                try {
                    await invoke("delete_credentials");
                    if (username) username.value = "";
                    if (password) password.value = "";
                    showNotification("Credentials deleted successfully");
                    checkCredentialsExist();
                    validateForm();
                } catch (err) {
                    showNotification("Failed to delete credentials", true);
                    console.error("Error:", err);
                }
            });
        }

        // Handle cancel
        if (cancelBtn) {
            cancelBtn.addEventListener("click", async () => {
                hideTimerNotification();
                cancelAutoCloseTimer();
                await invoke("quit_app");
            });
        }

        // Handle form submit
        form.addEventListener("submit", async (e) => {
            hideTimerNotification();
            cancelAutoCloseTimer();
            e.preventDefault();
            try {
                await invoke("save_credentials", {
                    credentials: {
                        username: username?.value,
                        password: password?.value,
                    },
                });
                
                // Show success notification immediately after saving
                showNotification("Credentials saved successfully");
                
                // Enable delete button after successful save
                if (deleteBtn) {
                    deleteBtn.disabled = false;
                    deleteBtn.classList.remove("opacity-50", "cursor-not-allowed");
                }
                
                // Switch windows after a short delay to ensure notification is seen
                setTimeout(async () => {
                    await invoke("switch_to_main_window");
                }, 500);
                
            } catch (err) {
                showNotification("Failed to save credentials", true);
                console.error("Error:", err);
            }
        });
    }

    // Add this to your initialization code
    window.addEventListener('storage', (e) => {
        if (e.key === 'theme') {
            // Update theme when it's changed in another window
            const newTheme = e.newValue || 'dracula';
            document.documentElement.setAttribute('data-theme', newTheme);
        }
    });

    // Secret reset shortcut: Ctrl+Shift+Alt+R
    window.addEventListener('keydown', async (e) => {
        if (e.ctrlKey && e.shiftKey && e.altKey && e.key === 'R') {
            e.preventDefault();
            
            const confirmed = confirm(
                '⚠️ WARNING: Application Reset ⚠️\n\n' +
                'This will permanently delete:\n' +
                '• All saved credentials\n' +
                '• All RDP connection files\n' +
                '• All saved hosts\n\n' +
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

    // Modify the back button handler to properly switch windows
    const backToLogin = document.querySelector("#backToLogin");
    if (backToLogin) {
        backToLogin.addEventListener("click", async () => {
            try {
                // Set flag to prevent timer from starting
                isIntentionalReturn = true;
                // First show login window, then hide main window
                await invoke("show_login_window");
                await invoke("hide_main_window");
            } catch (err) {
                console.error("Error switching windows:", err);
            }
        });
    }

    // Update the manage hosts event listener
    document.getElementById("manageHosts")?.addEventListener("click", async () => {
        try {
            await invoke("show_hosts_window");
        } catch (err) {
            console.error("Error showing hosts window:", err);
        }
    });
});

