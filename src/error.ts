import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';

interface ErrorData {
    message: string;
    timestamp: string;
    category?: string;
    details?: string;
}

let errors: ErrorData[] = [];

// DOM Elements
const errorList = document.getElementById('errorList') as HTMLDivElement;
const closeBtn = document.getElementById('closeBtn') as HTMLButtonElement;
const clearBtn = document.getElementById('clearBtn') as HTMLButtonElement;
const errorCount = document.getElementById('errorCount') as HTMLDivElement;

// Apply theme on load
async function applyTheme() {
    try {
        const theme = await invoke<string>('get_theme');
        if (theme === 'dark') {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
    } catch (error) {
        console.error('Failed to get theme:', error);
    }
}

// Add error to the list
function addError(error: ErrorData) {
    errors.push(error);
    renderErrors();
}

// Render all errors
function renderErrors() {
    errorList.innerHTML = '';
    
    if (errors.length === 0) {
        errorList.innerHTML = `
            <div class="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500">
                <svg class="w-16 h-16 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <p class="text-lg font-medium">No errors to display</p>
                <p class="text-sm mt-1">All errors have been cleared</p>
            </div>
        `;
        errorCount.textContent = '0 error(s)';
        return;
    }
    
    // Render errors in reverse order (newest first)
    errors.slice().reverse().forEach((error, index) => {
        const errorDiv = document.createElement('div');
        errorDiv.className = 'p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg';
        
        errorDiv.innerHTML = `
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <div class="flex items-center space-x-2 mb-2">
                        <span class="text-xs font-semibold text-red-800 dark:text-red-300 px-2 py-1 bg-red-100 dark:bg-red-900/40 rounded">
                            ${error.category || 'ERROR'}
                        </span>
                        <span class="text-xs text-gray-500 dark:text-gray-400">
                            ${error.timestamp}
                        </span>
                    </div>
                    <p class="text-sm font-medium text-red-900 dark:text-red-200 mb-1 break-words">
                        ${escapeHtml(error.message)}
                    </p>
                    ${error.details ? `
                        <details class="mt-2">
                            <summary class="text-xs text-gray-600 dark:text-gray-400 cursor-pointer hover:text-gray-800 dark:hover:text-gray-200">
                                Show details
                            </summary>
                            <pre class="mt-2 p-2 bg-gray-100 dark:bg-gray-900 rounded text-xs text-gray-700 dark:text-gray-300 overflow-x-auto whitespace-pre-wrap break-words">${escapeHtml(error.details)}</pre>
                        </details>
                    ` : ''}
                </div>
                <button class="ml-4 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors" onclick="removeError(${errors.length - 1 - index})">
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
        `;
        
        errorList.appendChild(errorDiv);
    });
    
    errorCount.textContent = `${errors.length} error(s)`;
}

// Remove specific error
(window as any).removeError = (index: number) => {
    errors.splice(index, 1);
    renderErrors();
};

// Escape HTML to prevent XSS
function escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Clear all errors
clearBtn.addEventListener('click', () => {
    errors = [];
    renderErrors();
});

// Close window
closeBtn.addEventListener('click', async () => {
    // Don't clear errors, just hide the window
    await getCurrentWindow().hide();
});

// Listen for error events from backend
listen<ErrorData>('show-error', (event: any) => {
    addError(event.payload);
    // The window will be shown automatically by the backend
});

// Listen for theme changes
listen<string>('theme-changed', (event: any) => {
    if (event.payload === 'dark') {
        document.documentElement.classList.add('dark');
    } else {
        document.documentElement.classList.remove('dark');
    }
});

// Handle Escape key to close
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        // Don't clear errors, just hide the window
        getCurrentWindow().hide();
    }
});

// Initialize
applyTheme();
renderErrors();
