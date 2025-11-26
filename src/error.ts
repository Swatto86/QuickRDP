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
let filteredErrors: ErrorData[] = [];
let searchQuery = '';
let autoScroll = true;

// DOM Elements
const errorList = document.getElementById('errorList') as HTMLDivElement;
const closeBtn = document.getElementById('closeBtn') as HTMLButtonElement;
const clearBtn = document.getElementById('clearBtn') as HTMLButtonElement;
const exportBtn = document.getElementById('exportBtn') as HTMLButtonElement;
const errorCount = document.getElementById('errorCount') as HTMLDivElement;
const searchInput = document.getElementById('searchInput') as HTMLInputElement;
const clearSearchBtn = document.getElementById('clearSearchBtn') as HTMLButtonElement;
const autoScrollCheckbox = document.getElementById('autoScrollCheckbox') as HTMLInputElement;
const filteredCount = document.getElementById('filteredCount') as HTMLDivElement;

// Add error to the list
function addError(error: ErrorData) {
    errors.push(error);
    applyFilters();
    renderErrors();
    
    // Auto-scroll to bottom if enabled
    if (autoScroll) {
        setTimeout(() => {
            errorList.scrollTop = errorList.scrollHeight;
        }, 100);
    }
}

// Apply search filters
function applyFilters() {
    if (!searchQuery.trim()) {
        filteredErrors = [...errors];
    } else {
        const query = searchQuery.toLowerCase();
        filteredErrors = errors.filter(error => 
            error.message.toLowerCase().includes(query) ||
            error.category?.toLowerCase().includes(query) ||
            error.details?.toLowerCase().includes(query) ||
            error.timestamp.toLowerCase().includes(query)
        );
    }
    updateFilteredCount();
}

// Update filtered count display
function updateFilteredCount() {
    if (searchQuery.trim() && filteredErrors.length !== errors.length) {
        filteredCount.textContent = `Showing ${filteredErrors.length} of ${errors.length}`;
    } else {
        filteredCount.textContent = '';
    }
}

// Get severity badge color based on category
function getSeverityColor(category?: string): string {
    const cat = category?.toUpperCase() || 'ERROR';
    if (cat.includes('CRITICAL') || cat.includes('FATAL')) {
        return 'bg-purple-100 dark:bg-purple-900/40 text-purple-800 dark:text-purple-300';
    } else if (cat.includes('ERROR')) {
        return 'bg-red-100 dark:bg-red-900/40 text-red-800 dark:text-red-300';
    } else if (cat.includes('WARN')) {
        return 'bg-yellow-100 dark:bg-yellow-900/40 text-yellow-800 dark:text-yellow-300';
    } else if (cat.includes('INFO')) {
        return 'bg-blue-100 dark:bg-blue-900/40 text-blue-800 dark:text-blue-300';
    }
    return 'bg-red-100 dark:bg-red-900/40 text-red-800 dark:text-red-300';
}

// Get border color based on category
function getBorderColor(category?: string): string {
    const cat = category?.toUpperCase() || 'ERROR';
    if (cat.includes('CRITICAL') || cat.includes('FATAL')) {
        return 'border-purple-200 dark:border-purple-800 bg-purple-50 dark:bg-purple-900/20';
    } else if (cat.includes('ERROR')) {
        return 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20';
    } else if (cat.includes('WARN')) {
        return 'border-yellow-200 dark:border-yellow-800 bg-yellow-50 dark:bg-yellow-900/20';
    } else if (cat.includes('INFO')) {
        return 'border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20';
    }
    return 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20';
}

// Render all errors
function renderErrors() {
    errorList.innerHTML = '';
    
    const errorsToDisplay = filteredErrors.length > 0 ? filteredErrors : errors;
    
    if (errors.length === 0) {
        errorList.innerHTML = `
            <div class="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500">
                <svg class="w-20 h-20 mb-4 animate-bounce" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <p class="text-lg font-medium">No errors to display</p>
                <p class="text-sm mt-1">All clear! No errors have been logged.</p>
            </div>
        `;
        errorCount.textContent = '0 error(s)';
        return;
    }
    
    if (searchQuery.trim() && filteredErrors.length === 0) {
        errorList.innerHTML = `
            <div class="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500">
                <svg class="w-16 h-16 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <p class="text-lg font-medium">No matching errors</p>
                <p class="text-sm mt-1">Try a different search term</p>
            </div>
        `;
        errorCount.textContent = `${errors.length} error(s)`;
        return;
    }
    
    // Render errors in reverse order (newest first)
    errorsToDisplay.slice().reverse().forEach((error) => {
        const actualIndex = errors.indexOf(error);
        const errorDiv = document.createElement('div');
        errorDiv.className = `p-4 border rounded-lg transition-all duration-200 hover:shadow-md ${getBorderColor(error.category)}`;
        
        errorDiv.innerHTML = `
            <div class="flex items-start justify-between">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center space-x-2 mb-2 flex-wrap">
                        <span class="text-xs font-semibold px-2 py-1 rounded ${getSeverityColor(error.category)}">
                            ${error.category || 'ERROR'}
                        </span>
                        <span class="text-xs text-gray-500 dark:text-gray-400 flex items-center">
                            <svg class="w-3 h-3 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                            ${error.timestamp}
                        </span>
                    </div>
                    <p class="text-sm font-medium text-gray-900 dark:text-gray-100 mb-1 break-words">
                        ${escapeHtml(error.message)}
                    </p>
                    ${error.details ? `
                        <details class="mt-2 group">
                            <summary class="text-xs text-gray-600 dark:text-gray-400 cursor-pointer hover:text-gray-800 dark:hover:text-gray-200 flex items-center space-x-1">
                                <svg class="w-3 h-3 transform transition-transform group-open:rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                                </svg>
                                <span>Show details</span>
                            </summary>
                            <pre class="mt-2 p-3 bg-gray-100 dark:bg-gray-900 rounded text-xs text-gray-700 dark:text-gray-300 overflow-x-auto whitespace-pre-wrap break-words border border-gray-200 dark:border-gray-700">${escapeHtml(error.details)}</pre>
                        </details>
                    ` : ''}
                </div>
                <div class="flex items-start space-x-2 ml-4">
                    <button class="text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors" onclick="copyError(${actualIndex})" title="Copy to clipboard">
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                        </svg>
                    </button>
                    <button class="text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors" onclick="removeError(${actualIndex})" title="Remove error">
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>
            </div>
        `;
        
        errorList.appendChild(errorDiv);
    });
    
    errorCount.textContent = `${errors.length} error(s)`;
}

// Remove specific error
(window as any).removeError = (index: number) => {
    errors.splice(index, 1);
    applyFilters();
    renderErrors();
};

// Copy error to clipboard
(window as any).copyError = async (index: number) => {
    const error = errors[index];
    if (!error) return;
    
    const text = `[${error.category || 'ERROR'}] ${error.timestamp}\n${error.message}${error.details ? '\n\nDetails:\n' + error.details : ''}`;
    
    try {
        await navigator.clipboard.writeText(text);
        showNotification('Error copied to clipboard');
    } catch (err) {
        console.error('Failed to copy:', err);
        showNotification('Failed to copy to clipboard', true);
    }
};

// Show temporary notification
function showNotification(message: string, isError = false) {
    const notification = document.createElement('div');
    notification.className = `fixed bottom-4 right-4 px-4 py-2 rounded-lg shadow-lg text-white text-sm font-medium transition-opacity duration-300 ${isError ? 'bg-red-600' : 'bg-green-600'}`;
    notification.textContent = message;
    notification.style.opacity = '0';
    document.body.appendChild(notification);
    
    setTimeout(() => notification.style.opacity = '1', 10);
    setTimeout(() => {
        notification.style.opacity = '0';
        setTimeout(() => notification.remove(), 300);
    }, 2000);
}

// Escape HTML to prevent XSS
function escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Clear all errors
clearBtn.addEventListener('click', () => {
    if (errors.length === 0) return;
    
    if (confirm(`Are you sure you want to clear all ${errors.length} error(s)?`)) {
        errors = [];
        filteredErrors = [];
        searchQuery = '';
        searchInput.value = '';
        clearSearchBtn.classList.add('hidden');
        applyFilters();
        renderErrors();
        showNotification('All errors cleared');
    }
});

// Export errors to clipboard
exportBtn.addEventListener('click', async () => {
    if (errors.length === 0) {
        showNotification('No errors to export', true);
        return;
    }
    
    const text = errors.map(error => 
        `[${error.category || 'ERROR'}] ${error.timestamp}\n${error.message}${error.details ? '\n\nDetails:\n' + error.details : ''}\n${'='.repeat(80)}`
    ).join('\n\n');
    
    try {
        await navigator.clipboard.writeText(text);
        showNotification(`Exported ${errors.length} error(s) to clipboard`);
    } catch (err) {
        console.error('Failed to export:', err);
        showNotification('Failed to export to clipboard', true);
    }
});

// Search functionality
searchInput.addEventListener('input', (e) => {
    searchQuery = (e.target as HTMLInputElement).value;
    clearSearchBtn.classList.toggle('hidden', !searchQuery.trim());
    applyFilters();
    renderErrors();
});

clearSearchBtn.addEventListener('click', () => {
    searchQuery = '';
    searchInput.value = '';
    clearSearchBtn.classList.add('hidden');
    applyFilters();
    renderErrors();
});

// Auto-scroll toggle
autoScrollCheckbox.addEventListener('change', (e) => {
    autoScroll = (e.target as HTMLInputElement).checked;
});

// Close/Hide window
closeBtn.addEventListener('click', async () => {
    // Don't clear errors, just hide the window
    await getCurrentWindow().hide();
});

// Handle keyboard shortcuts
document.addEventListener('keydown', async (e) => {
    // Ctrl+Shift+E to toggle window
    if (e.ctrlKey && e.shiftKey && e.key === 'E') {
        e.preventDefault();
        await getCurrentWindow().hide();
    }
    
    // Escape key to close
    if (e.key === 'Escape') {
        e.preventDefault();
        await getCurrentWindow().hide();
    }
});

// Initialize theme and listeners
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

// Listen for error events from backend and initialize
(async () => {
    await initializeTheme();
    
    await listen<ErrorData>('show-error', (event: any) => {
        addError(event.payload);
        // The window will be shown automatically by the backend
    });

    renderErrors();
})();
