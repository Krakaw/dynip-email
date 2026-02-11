// Application state
let currentAddress = '';
let websocket = null;
let emails = [];
let selectedEmailId = null;
let webhooks = [];
let currentTab = 'emails';
let mailboxPassword = ''; // Current session password

// WebSocket reconnection state
let reconnectAttempts = 0;
let reconnectTimeout = null;
let isPageVisible = true;
let shouldReconnect = false;

// Favicon management
let originalFavicon = null;

// Authentication state
let authEnabled = false;
let authToken = null;
let currentUser = null;

// DOM elements
const emailAddressInput = document.getElementById('emailAddress');
const loadEmailsBtn = document.getElementById('loadEmails');
const emailList = document.getElementById('emailList');
const webhookList = document.getElementById('webhookList');
const emailDetail = document.getElementById('emailDetail');
const connectionStatus = document.getElementById('connectionStatus');
const emailCount = document.getElementById('emailCount');
const themeToggle = document.getElementById('themeToggle');
const themeIcon = document.getElementById('themeIcon');
const inboxTab = document.getElementById('inboxTab');
const webhooksTab = document.getElementById('webhooksTab');

// Event listeners
loadEmailsBtn.addEventListener('click', loadInbox);
emailAddressInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') loadInbox();
});
themeToggle.addEventListener('click', toggleTheme);
inboxTab.addEventListener('click', () => switchTab('emails'));
webhooksTab.addEventListener('click', () => switchTab('webhooks'));

// Page visibility and focus event listeners
document.addEventListener('visibilitychange', handleVisibilityChange);
window.addEventListener('focus', handleWindowFocus);

// Theme management
function initTheme() {
    const savedTheme = localStorage.getItem('theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    
    if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
        enableDarkMode();
    } else {
        enableLightMode();
    }
}

function toggleTheme() {
    const isDark = document.documentElement.classList.contains('dark-mode');
    if (isDark) {
        enableLightMode();
    } else {
        enableDarkMode();
    }
}

function enableDarkMode() {
    document.documentElement.classList.add('dark-mode');
    themeIcon.textContent = '☀️';
    localStorage.setItem('theme', 'dark');
}

function enableLightMode() {
    document.documentElement.classList.remove('dark-mode');
    themeIcon.textContent = '🌙';
    localStorage.setItem('theme', 'light');
}

// Initialize from URL on page load
window.addEventListener('DOMContentLoaded', async () => {
    // Initialize theme
    initTheme();
    
    // Check auth status and initialize
    await initAuth();
    
    // Load mailbox from URL if present
    const urlParams = new URLSearchParams(window.location.search);
    const mailbox = urlParams.get('mailbox');
    
    if (mailbox) {
        emailAddressInput.value = mailbox;
        loadInbox();
    }
});

// ============================================================================
// Authentication Functions
// ============================================================================

// Initialize authentication
async function initAuth() {
    // Load stored token
    authToken = localStorage.getItem('auth_token');
    
    // Check auth status from server
    try {
        const response = await fetch('/api/auth/status');
        const status = await response.json();
        
        authEnabled = status.auth_enabled;
        authDomain = status.auth_domain || null;
        
        if (authEnabled) {
            // Show auth UI elements
            showAuthUI();
            
            if (authToken) {
                // Verify token is still valid
                const valid = await verifyAuthToken();
                if (!valid) {
                    authToken = null;
                    localStorage.removeItem('auth_token');
                    showLoginForm();
                } else {
                    updateAuthUI();
                }
            } else if (status.registration_open) {
                // No users yet, show registration
                showRegisterForm();
            } else {
                // Show login form
                showLoginForm();
            }
        }
    } catch (error) {
        console.error('Failed to check auth status:', error);
    }
}

// Verify auth token is valid
async function verifyAuthToken() {
    if (!authToken) return false;
    
    try {
        const response = await fetch('/api/auth/me', {
            headers: {
                'Authorization': `Bearer ${authToken}`
            }
        });
        
        if (response.ok) {
            currentUser = await response.json();
            return true;
        }
        return false;
    } catch (error) {
        return false;
    }
}

// Show auth UI elements in header
function showAuthUI() {
    const header = document.querySelector('header');
    
    // Check if auth container already exists
    if (document.getElementById('authContainer')) return;
    
    const authContainer = document.createElement('div');
    authContainer.id = 'authContainer';
    authContainer.className = 'auth-container';
    authContainer.innerHTML = `
        <div id="authStatus" class="auth-status"></div>
    `;
    
    // Insert after theme toggle
    const themeToggle = document.getElementById('themeToggle');
    themeToggle.parentNode.insertBefore(authContainer, themeToggle);
}

// Update auth UI based on current state
function updateAuthUI() {
    const authStatus = document.getElementById('authStatus');
    if (!authStatus) return;
    
    if (currentUser) {
        authStatus.innerHTML = `
            <span class="user-info">👤 ${escapeHtml(currentUser.email)}</span>
            <button class="btn-secondary btn-small" onclick="logout()">Logout</button>
        `;
    } else {
        authStatus.innerHTML = `
            <button class="btn-primary btn-small" onclick="showLoginForm()">Login</button>
        `;
    }
}

// Show login form modal
function showLoginForm() {
    closeAuthModal();
    
    const modal = document.createElement('div');
    modal.className = 'auth-modal';
    modal.id = 'authModal';
    modal.innerHTML = `
        <div class="auth-modal-content">
            <h2>🔐 Login</h2>
            <form id="loginForm">
                <div class="form-group">
                    <label for="loginEmail">Email:</label>
                    <input type="email" id="loginEmail" placeholder="Enter your email" required autofocus>
                </div>
                <div class="form-group">
                    <label for="loginPassword">Password:</label>
                    <input type="password" id="loginPassword" placeholder="Enter password" required>
                </div>
                <div id="loginError" class="form-error" style="display: none;"></div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Login</button>
                    <button type="button" class="btn-secondary" onclick="showRegisterForm()">Register</button>
                </div>
            </form>
        </div>
    `;
    
    document.body.appendChild(modal);
    
    document.getElementById('loginForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await handleLogin();
    });
}

// Auth domain restriction (set from server)
let authDomain = null;

// Show register form modal
function showRegisterForm() {
    closeAuthModal();
    
    const domainHint = authDomain ? ` (must be @${authDomain})` : '';
    const emailPlaceholder = authDomain ? `user@${authDomain}` : 'your@email.com';
    
    const modal = document.createElement('div');
    modal.className = 'auth-modal';
    modal.id = 'authModal';
    modal.innerHTML = `
        <div class="auth-modal-content">
            <h2>📝 Register</h2>
            <form id="registerForm">
                <div class="form-group">
                    <label for="registerEmail">Email${domainHint}:</label>
                    <input type="email" id="registerEmail" placeholder="${emailPlaceholder}" required autofocus>
                </div>
                <div class="form-group">
                    <label for="registerPassword">Password:</label>
                    <input type="password" id="registerPassword" placeholder="At least 8 characters" required minlength="8">
                </div>
                <div class="form-group">
                    <label for="registerConfirm">Confirm Password:</label>
                    <input type="password" id="registerConfirm" placeholder="Confirm password" required>
                </div>
                <div id="registerError" class="form-error" style="display: none;"></div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Register</button>
                    <button type="button" class="btn-secondary" onclick="showLoginForm()">Back to Login</button>
                </div>
            </form>
        </div>
    `;
    
    document.body.appendChild(modal);
    
    document.getElementById('registerForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await handleRegister();
    });
}

// Close auth modal
function closeAuthModal() {
    const modal = document.getElementById('authModal');
    if (modal) modal.remove();
}

// Handle login form submission
async function handleLogin() {
    const email = document.getElementById('loginEmail').value;
    const password = document.getElementById('loginPassword').value;
    const errorDiv = document.getElementById('loginError');
    
    try {
        const response = await fetch('/api/auth/login', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ email, password })
        });
        
        if (response.ok) {
            const data = await response.json();
            authToken = data.token;
            currentUser = data.user;
            localStorage.setItem('auth_token', authToken);
            closeAuthModal();
            updateAuthUI();
        } else {
            const error = await response.text();
            errorDiv.textContent = error || 'Login failed';
            errorDiv.style.display = 'block';
        }
    } catch (error) {
        console.error('Login error:', error);
        errorDiv.textContent = 'Login failed. Please try again.';
        errorDiv.style.display = 'block';
    }
}

// Handle register form submission
async function handleRegister() {
    const email = document.getElementById('registerEmail').value;
    const password = document.getElementById('registerPassword').value;
    const confirm = document.getElementById('registerConfirm').value;
    const errorDiv = document.getElementById('registerError');
    
    if (password !== confirm) {
        errorDiv.textContent = 'Passwords do not match';
        errorDiv.style.display = 'block';
        return;
    }
    
    try {
        const response = await fetch('/api/auth/register', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ email, password })
        });
        
        if (response.ok) {
            const data = await response.json();
            authToken = data.token;
            currentUser = data.user;
            localStorage.setItem('auth_token', authToken);
            closeAuthModal();
            updateAuthUI();
        } else {
            const error = await response.text();
            errorDiv.textContent = error || 'Registration failed';
            errorDiv.style.display = 'block';
        }
    } catch (error) {
        console.error('Registration error:', error);
        errorDiv.textContent = 'Registration failed. Please try again.';
        errorDiv.style.display = 'block';
    }
}

// Logout
function logout() {
    authToken = null;
    currentUser = null;
    localStorage.removeItem('auth_token');
    updateAuthUI();
    showLoginForm();
}

// Get auth headers for API calls
function getAuthHeaders() {
    const headers = {
        'Content-Type': 'application/json',
    };
    
    if (authEnabled && authToken) {
        headers['Authorization'] = `Bearer ${authToken}`;
    }
    
    return headers;
}

// Wrapper for fetch that adds auth headers
async function authFetch(url, options = {}) {
    if (!options.headers) {
        options.headers = {};
    }
    
    if (authEnabled && authToken) {
        options.headers['Authorization'] = `Bearer ${authToken}`;
    }
    
    const response = await fetch(url, options);
    
    // If we get 401, token might be expired
    if (response.status === 401 && authEnabled) {
        authToken = null;
        currentUser = null;
        localStorage.removeItem('auth_token');
        updateAuthUI();
        showLoginForm();
        throw new Error('Authentication required');
    }
    
    return response;
}

// Password management
function getStoredPassword(address) {
    const key = `mailbox_password_${address}`;
    return localStorage.getItem(key) || '';
}

function storePassword(address, password) {
    const key = `mailbox_password_${address}`;
    localStorage.setItem(key, password);
    mailboxPassword = password;
}

function clearStoredPassword(address) {
    const key = `mailbox_password_${address}`;
    localStorage.removeItem(key);
    mailboxPassword = '';
}

// Check mailbox status (locked or unlocked)
async function checkMailboxStatus(address) {
    try {
        console.log('Checking mailbox status for:', address);
        const response = await authFetch(`/api/mailbox/${encodeURIComponent(address)}/status`);
        if (!response.ok) {
            console.error('Mailbox status check failed:', response.status);
            return false;
        }
        const data = await response.json();
        console.log('Mailbox status:', data);
        return data.is_locked;
    } catch (error) {
        console.error('Failed to check mailbox status:', error);
        return false;
    }
}

// Show claim mailbox modal
function showClaimModal(address) {
    const modal = document.createElement('div');
    modal.className = 'password-modal';
    modal.innerHTML = `
        <div class="password-modal-content">
            <h2>🔒 Claim This Mailbox</h2>
            <p>This mailbox is unclaimed. Set a password to protect it from unauthorized access.</p>
            <form id="claimForm">
                <div class="form-group">
                    <label for="claimPassword">Password:</label>
                    <input type="password" id="claimPassword" placeholder="Enter a strong password" required autofocus>
                </div>
                <div class="form-group">
                    <label for="confirmPassword">Confirm Password:</label>
                    <input type="password" id="confirmPassword" placeholder="Confirm password" required>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Claim Mailbox</button>
                    <button type="button" class="btn-secondary" onclick="closeClaimModal()">Skip</button>
                </div>
            </form>
        </div>
    `;
    
    document.body.appendChild(modal);
    
    document.getElementById('claimForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        const password = document.getElementById('claimPassword').value;
        const confirm = document.getElementById('confirmPassword').value;
        
        if (password !== confirm) {
            alert('Passwords do not match');
            return;
        }
        
        await claimMailbox(address, password);
    });
}

// Close claim modal
function closeClaimModal() {
    const modal = document.querySelector('.password-modal');
    if (modal) modal.remove();
    // Continue without claiming
    continueLoadInbox(currentAddress, '');
}

// Claim mailbox with password
async function claimMailbox(address, password) {
    try {
        const response = await authFetch(`/api/mailbox/${encodeURIComponent(address)}/claim`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ password })
        });
        
        if (response.ok) {
            storePassword(address, password);
            closeClaimModal();
            updateClaimStatus(true);
            continueLoadInbox(address, password);
        } else {
            const error = await response.json();
            alert('Failed to claim mailbox: ' + (error.message || 'Unknown error'));
        }
    } catch (error) {
        console.error('Failed to claim mailbox:', error);
        alert('Failed to claim mailbox');
    }
}

// Show unlock mailbox modal
function showUnlockModal(address) {
    const modal = document.createElement('div');
    modal.className = 'password-modal';
    modal.innerHTML = `
        <div class="password-modal-content">
            <h2>🔐 Mailbox Protected</h2>
            <p>This mailbox is password protected. Enter the password to access it.</p>
            <form id="unlockForm">
                <div class="form-group">
                    <label for="unlockPassword">Password:</label>
                    <input type="password" id="unlockPassword" placeholder="Enter password" required autofocus>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Unlock</button>
                    <button type="button" class="btn-secondary" onclick="closeUnlockModal()">Cancel</button>
                </div>
            </form>
        </div>
    `;
    
    document.body.appendChild(modal);
    
    document.getElementById('unlockForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        const password = document.getElementById('unlockPassword').value;
        await unlockMailbox(address, password);
    });
}

// Close unlock modal
function closeUnlockModal() {
    const modal = document.querySelector('.password-modal');
    if (modal) modal.remove();
}

// Unlock mailbox with password
async function unlockMailbox(address, password) {
    // Try to fetch emails with the password
    try {
        const response = await authFetch(`/api/emails/${encodeURIComponent(address)}?password=${encodeURIComponent(password)}`);
        
        if (response.ok) {
            storePassword(address, password);
            closeUnlockModal();
            updateClaimStatus(true);
            continueLoadInbox(address, password);
        } else if (response.status === 401) {
            alert('Incorrect password');
        } else {
            alert('Failed to unlock mailbox');
        }
    } catch (error) {
        console.error('Failed to unlock mailbox:', error);
        alert('Failed to unlock mailbox');
    }
}

// Load inbox for the specified email address
async function loadInbox() {
    const address = emailAddressInput.value.trim();
    console.log('loadInbox called, address:', address, 'authEnabled:', authEnabled, 'hasToken:', !!authToken);
    
    if (!address) {
        alert('Please enter an address');
        return;
    }
    
    currentAddress = address;
    
    // Update URL with mailbox query parameter
    const url = new URL(window.location);
    url.searchParams.set('mailbox', address);
    window.history.pushState({}, '', url);
    
    // Close existing WebSocket connection
    if (websocket) {
        stopReconnection(); // Stop any pending reconnection attempts
        websocket.close();
    }
    
    // Check if mailbox is locked
    const isLocked = await checkMailboxStatus(address);
    
    // Try to get stored password
    const storedPassword = getStoredPassword(address);
    
    if (isLocked) {
        // Mailbox is claimed - need password
        if (storedPassword) {
            // Try stored password
            mailboxPassword = storedPassword;
            const passwordWorks = await verifyPassword(address, storedPassword);
            if (passwordWorks) {
                updateClaimStatus(true);
                continueLoadInbox(address, storedPassword);
            } else {
                // Stored password is wrong, clear it and ask for new one
                clearStoredPassword(address);
                showUnlockModal(address);
            }
        } else {
            // No stored password, ask user
            showUnlockModal(address);
        }
    } else {
        // Mailbox is unclaimed - load directly
        updateClaimStatus(false);
        mailboxPassword = storedPassword;
        continueLoadInbox(address, storedPassword);
    }
}

// Update claim/release button in status bar
function updateClaimStatus(isClaimed) {
    const lockBtn = document.getElementById('lockMailbox');
    if (!lockBtn) return; // Element might not exist in some contexts
    
    if (!currentAddress) {
        lockBtn.style.display = 'none';
        return;
    }
    lockBtn.style.display = '';
    if (isClaimed) {
        lockBtn.textContent = 'Release';
        lockBtn.className = 'lock-btn release';
    } else {
        lockBtn.textContent = 'Claim';
        lockBtn.className = 'lock-btn claim';
    }
}

// Claim/release button click handler
const lockMailboxBtn = document.getElementById('lockMailbox');
if (lockMailboxBtn) {
    lockMailboxBtn.addEventListener('click', async () => {
        if (!currentAddress) return;
        const isLocked = await checkMailboxStatus(currentAddress);
        if (isLocked) {
            await releaseMailbox(currentAddress);
        } else {
            showClaimModal(currentAddress);
        }
    });
}

// Release mailbox (remove password protection)
async function releaseMailbox(address) {
    if (!mailboxPassword) {
        alert('No active session password to authorize release.');
        return;
    }
    if (!confirm('Release this mailbox? It will become publicly accessible and anyone could claim it.')) {
        return;
    }
    try {
        const response = await authFetch(`/api/mailbox/${encodeURIComponent(address)}/release`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ password: mailboxPassword })
        });
        if (response.ok) {
            clearStoredPassword(address);
            updateClaimStatus(false);
        } else {
            const error = await response.text();
            alert('Failed to release mailbox: ' + error);
        }
    } catch (error) {
        console.error('Failed to release mailbox:', error);
        alert('Failed to release mailbox');
    }
}

// Verify if password is correct
async function verifyPassword(address, password) {
    try {
        const response = await authFetch(`/api/emails/${encodeURIComponent(address)}?password=${encodeURIComponent(password)}`);
        return response.ok;
    } catch (error) {
        return false;
    }
}

// Continue loading inbox after password handling
async function continueLoadInbox(address, password) {
    mailboxPassword = password;
    
    // Show loading state
    emailList.innerHTML = '<div class="empty-state"><p>⏳ Loading emails...</p></div>';
    
    // Fetch emails
    try {
        const passwordParam = password ? `?password=${encodeURIComponent(password)}` : '';
        console.log('Fetching emails for:', address, 'with auth:', !!authToken);
        const response = await authFetch(`/api/emails/${encodeURIComponent(address)}${passwordParam}`);
        
        if (!response.ok) {
            const errorText = await response.text();
            console.error('API error:', response.status, errorText);
            throw new Error(`HTTP ${response.status}: ${errorText}`);
        }
        
        const data = await response.json();
        console.log('Loaded emails:', data.emails?.length || 0);
        
        emails = data.emails || [];
        displayEmails(emails);
        updateEmailCount(emails.length);
        
        // Connect WebSocket
        connectWebSocket(address);
        
        // Load webhooks for this address
        loadWebhooks(address);
    } catch (error) {
        console.error('Failed to load emails:', error);
        emailList.innerHTML = `<div class="empty-state"><p>❌ Failed to load emails</p><p style="font-size: 0.875rem; color: var(--text-secondary);">${escapeHtml(error.message)}</p></div>`;
    }
}

// Display emails in the list
function displayEmails(emailsToDisplay) {
    if (emailsToDisplay.length === 0) {
        emailList.innerHTML = '<div class="empty-state"><p>📭 No emails yet</p></div>';
        updateFavicon(); // Update favicon when no emails
        return;
    }
    
    emailList.innerHTML = emailsToDisplay
        .map(email => {
            const date = new Date(email.timestamp);
            const isNew = email.isNew || false;
            
            return `
                <div class="email-item ${email.id === selectedEmailId ? 'active' : ''}" 
                     data-email-id="${email.id}"
                     onclick="showEmailDetail('${email.id}')">
                    <div class="from">
                        ${escapeHtml(email.from)}
                        ${isNew ? '<span class="new-badge">NEW</span>' : ''}
                    </div>
                    <div class="subject">${escapeHtml(email.subject)}</div>
                    <div class="timestamp">${formatDate(date)}</div>
                </div>
            `;
        })
        .join('');
    
    // Update favicon based on unread status
    updateFavicon();
}

// Show email detail
function showEmailDetail(emailId) {
    const email = emails.find(e => e.id === emailId);
    if (!email) return;
    
    selectedEmailId = emailId;
    
    // Update active state in list
    document.querySelectorAll('.email-item').forEach(item => {
        item.classList.remove('active');
    });
    document.querySelector(`[data-email-id="${emailId}"]`)?.classList.add('active');
    
    // Display email detail
    const date = new Date(email.timestamp);
    
    // Determine if body is HTML or plain text
    const isHtml = email.body.includes('<') && email.body.includes('>');
    const bodyContent = isHtml
        ? `<iframe srcdoc="${escapeHtml(email.body)}"></iframe>`
        : `<pre class="email-body-text">${escapeHtml(email.body)}</pre>`;
    
    // Build attachments HTML if any
    let attachmentsHtml = '';
    if (email.attachments && email.attachments.length > 0) {
        attachmentsHtml = `
            <div class="email-attachments">
                <h3>📎 Attachments (${email.attachments.length})</h3>
                <div class="attachments-list">
                    ${email.attachments.map(att => `
                        <div class="attachment-item">
                            <div class="attachment-info">
                                <span class="attachment-icon">${getFileIcon(att.content_type)}</span>
                                <div class="attachment-details">
                                    <div class="attachment-name">${escapeHtml(att.filename)}</div>
                                    <div class="attachment-meta">${formatFileSize(att.size)} • ${att.content_type}</div>
                                </div>
                            </div>
                            <button class="attachment-download" onclick="downloadAttachment('${email.id}', '${escapeHtml(att.filename)}', '${att.content_type}', '${att.content}')">
                                Download
                            </button>
                        </div>
                    `).join('')}
                </div>
            </div>
        `;
    }
    
    emailDetail.innerHTML = `
        <div class="email-header">
            <div class="email-header-top">
                <h2 class="email-subject">${escapeHtml(email.subject)}</h2>
                <div class="email-actions">
                    <button class="delete-email-btn" onclick="deleteEmail('${email.id}')" title="Delete Email">
                        🗑️ Delete
                    </button>
                </div>
            </div>
            <div class="email-meta">
                <div class="email-meta-item">
                    <span class="email-meta-label">From:</span>
                    <span>${escapeHtml(email.from)}</span>
                </div>
                <div class="email-meta-item">
                    <span class="email-meta-label">To:</span>
                    <span>${escapeHtml(email.to)}</span>
                </div>
                <div class="email-meta-item">
                    <span class="email-meta-label">Date:</span>
                    <span>${formatDateFull(date)}</span>
                </div>
            </div>
        </div>
        <div class="email-body">
            ${bodyContent}
        </div>
        ${attachmentsHtml}
    `;
    
    // Mark as read (remove new badge)
    if (email.isNew) {
        email.isNew = false;
        displayEmails(emails);
        updateFavicon(); // Update favicon when email is marked as read
    }
}

// Connect to WebSocket for real-time updates
function connectWebSocket(address) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/ws/${encodeURIComponent(address)}`;
    
    // Clear any existing reconnection timeout
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
    }
    
    websocket = new WebSocket(wsUrl);
    shouldReconnect = true;
    
    websocket.onopen = () => {
        console.log('WebSocket connected');
        reconnectAttempts = 0; // Reset attempts on successful connection
        updateConnectionStatus(true);
    };
    
    websocket.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            
            if (data.type === 'Connected') {
                console.log('Connected to WebSocket for:', data.address);
                return;
            }
            
            // New email received
            if (data.type === 'Email') {
                const email = data;
                console.log('New email received:', email);
                email.isNew = true;
                emails.unshift(email);
                displayEmails(emails);
                updateEmailCount(emails.length);
                
                // Show enhanced notification
                showEnhancedNotification(email);
            }
            
            // Email deleted
            if (data.type === 'EmailDeleted') {
                const { id, address } = data;
                console.log('Email deleted:', id, 'for address:', address);
                
                // Remove the email from the local array
                const initialLength = emails.length;
                emails = emails.filter(email => email.id !== id);
                
                // Update the display if the email count changed
                if (emails.length !== initialLength) {
                    displayEmails(emails);
                    updateEmailCount(emails.length);
                    
                    // If the deleted email was currently selected, clear the detail view
                    if (selectedEmailId === id) {
                        selectedEmailId = null;
                        emailDetail.innerHTML = '<div class="empty-state"><p>📭 Select an email to view</p></div>';
                        
                        // Remove active state from all email items
                        document.querySelectorAll('.email-item').forEach(item => {
                            item.classList.remove('active');
                        });
                    }
                    
                    // Show notification
                    showNotification('🗑️ Email deleted', 'An email was removed due to retention policy');
                }
            }
        } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
        }
    };
    
    websocket.onerror = (error) => {
        console.error('WebSocket error:', error);
        updateConnectionStatus(false);
    };
    
    websocket.onclose = () => {
        console.log('WebSocket disconnected');
        updateConnectionStatus(false);
        
        // Attempt to reconnect with exponential backoff
        if (shouldReconnect) {
            scheduleReconnect(address);
        }
    };
}

// Schedule reconnection with exponential backoff
function scheduleReconnect(address) {
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
    }
    
    const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 30000);
    reconnectAttempts++;
    
    console.log(`Scheduling reconnection attempt ${reconnectAttempts} in ${delay}ms`);
    updateConnectionStatus(false, 'Reconnecting...');
    
    reconnectTimeout = setTimeout(() => {
        if (shouldReconnect) {
            connectWebSocket(address);
        }
    }, delay);
}

// Stop reconnection attempts
function stopReconnection() {
    shouldReconnect = false;
    if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
        reconnectTimeout = null;
    }
    reconnectAttempts = 0;
}

// Handle page visibility change
function handleVisibilityChange() {
    const wasVisible = isPageVisible;
    isPageVisible = !document.hidden;
    
    // If page becomes visible after being hidden, check connection and refresh
    if (wasVisible === false && isPageVisible === true) {
        console.log('Page became visible, checking connection...');
        checkAndReconnectIfNeeded();
    }
}

// Handle window focus
function handleWindowFocus() {
    console.log('Window focused, checking connection...');
    checkAndReconnectIfNeeded();
}

// Check WebSocket connection and reconnect if needed
async function checkAndReconnectIfNeeded() {
    if (!currentAddress) return;
    
    // Check if WebSocket is closed or missing
    if (!websocket || websocket.readyState === WebSocket.CLOSED) {
        console.log('WebSocket is disconnected, reconnecting...');
        connectWebSocket(currentAddress);
        
        // Fetch latest emails to ensure we have the most recent state
        try {
            console.log('Fetching latest emails after reconnection...');
            const response = await authFetch(`/api/emails/${encodeURIComponent(currentAddress)}`);
            const data = await response.json();
            
            emails = data.emails || [];
            displayEmails(emails);
            updateEmailCount(emails.length);
        } catch (error) {
            console.error('Failed to fetch latest emails:', error);
        }
    }
}

// Update connection status indicator
function updateConnectionStatus(isConnected, statusText = null) {
    if (isConnected) {
        connectionStatus.classList.remove('offline');
        connectionStatus.classList.add('online');
        connectionStatus.querySelector('.status-text').textContent = 'Connected';
    } else {
        connectionStatus.classList.remove('online');
        connectionStatus.classList.add('offline');
        connectionStatus.querySelector('.status-text').textContent = statusText || 'Disconnected';
    }
}

// Update email count
function updateEmailCount(count) {
    emailCount.textContent = `${count} email${count !== 1 ? 's' : ''}`;
}

// Show browser notification
function showNotification(title, body) {
    if ('Notification' in window && Notification.permission === 'granted') {
        new Notification(title, { body });
    }
}

// Show enhanced notification for new emails
function showEnhancedNotification(email) {
    if ('Notification' in window && Notification.permission === 'granted') {
        const notification = new Notification('📬 New Email', {
            body: `From: ${email.from}\nSubject: ${email.subject}`,
            icon: '/logo.svg',
            tag: 'new-email'
        });
        
        notification.onclick = () => {
            window.focus();
            showEmailDetail(email.id);
            notification.close();
        };
    }
}

// Update favicon based on unread messages
function updateFavicon() {
    const hasUnread = emails.some(email => email.isNew);
    const favicon = document.querySelector('link[rel="icon"]') || document.querySelector('link[rel="shortcut icon"]');
    
    if (hasUnread) {
        // Store original favicon if not already stored
        if (!originalFavicon && favicon) {
            originalFavicon = favicon.href;
        }
        
        // Create red-tinted favicon using original
        setRedFavicon();
    } else {
        // Restore original favicon
        if (originalFavicon && favicon) {
            favicon.href = originalFavicon;
        }
    }
}

// Create red-tinted favicon using original favicon as base
function setRedFavicon() {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    canvas.width = 32;
    canvas.height = 32;
    
    // Create image from original favicon
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
        // Draw original favicon
        ctx.drawImage(img, 0, 0, 32, 32);
        
        // Apply red tint overlay
        ctx.globalCompositeOperation = 'multiply';
        ctx.fillStyle = '#ff4444';
        ctx.fillRect(0, 0, 32, 32);
        
        // Reset composite operation
        ctx.globalCompositeOperation = 'source-over';
        
        // Convert to data URL and set as favicon
        const dataURL = canvas.toDataURL('image/png');
        const favicon = document.querySelector('link[rel="icon"]') || document.querySelector('link[rel="shortcut icon"]');
        if (favicon) {
            favicon.href = dataURL;
        } else {
            // Create new favicon link if none exists
            const link = document.createElement('link');
            link.rel = 'icon';
            link.href = dataURL;
            document.head.appendChild(link);
        }
    };
    
    // Load original favicon
    img.src = originalFavicon || '/logo.svg';
}


// Request notification permission on page load
if ('Notification' in window && Notification.permission === 'default') {
    Notification.requestPermission();
}

// Utility functions
function escapeHtml(text) {
    const map = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#039;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
}

function formatDate(date) {
    const now = new Date();
    const diff = now - date;
    
    // Less than 1 minute
    if (diff < 60000) {
        return 'Just now';
    }
    
    // Less than 1 hour
    if (diff < 3600000) {
        const minutes = Math.floor(diff / 60000);
        return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;
    }
    
    // Less than 24 hours
    if (diff < 86400000) {
        const hours = Math.floor(diff / 3600000);
        return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
    }
    
    // More than 24 hours
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

function formatDateFull(date) {
    return date.toLocaleDateString('en-US', {
        weekday: 'long',
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
    });
}

// Download attachment
function downloadAttachment(emailId, filename, contentType, base64Content) {
    try {
        // Decode base64 to binary
        const binaryString = atob(base64Content);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        // Create blob and download
        const blob = new Blob([bytes], { type: contentType });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    } catch (error) {
        console.error('Failed to download attachment:', error);
        alert('Failed to download attachment');
    }
}

// Get file icon based on content type
function getFileIcon(contentType) {
    if (contentType.startsWith('image/')) return '🖼️';
    if (contentType.startsWith('video/')) return '🎥';
    if (contentType.startsWith('audio/')) return '🎵';
    if (contentType.includes('pdf')) return '📄';
    if (contentType.includes('zip') || contentType.includes('compressed')) return '📦';
    if (contentType.includes('text/')) return '📝';
    if (contentType.includes('word') || contentType.includes('document')) return '📃';
    if (contentType.includes('sheet') || contentType.includes('excel')) return '📊';
    if (contentType.includes('presentation') || contentType.includes('powerpoint')) return '📽️';
    return '📎';
}

// Format file size
function formatFileSize(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

// Delete email function
async function deleteEmail(emailId) {
    if (!confirm('Are you sure you want to delete this email? This action cannot be undone.')) {
        return;
    }
    
    try {
        const response = await authFetch(`/api/email/${emailId}`, {
            method: 'DELETE',
            headers: {
                'Content-Type': 'application/json',
            }
        });
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        // Remove email from local array
        emails = emails.filter(email => email.id !== emailId);
        
        // Clear email detail if this was the selected email
        if (selectedEmailId === emailId) {
            selectedEmailId = null;
            emailDetail.innerHTML = '<div class="no-email-selected">Select an email to view</div>';
        }
        
        // Refresh email list
        displayEmails(emails);
        updateFavicon(); // Update favicon after deletion
        
        // Show success message
        showNotification('Email deleted successfully', 'success');
        
    } catch (error) {
        console.error('Failed to delete email:', error);
        showNotification('Failed to delete email', 'error');
    }
}

// Tab switching
function switchTab(tab) {
    currentTab = tab;
    
    if (tab === 'emails') {
        inboxTab.classList.add('active');
        webhooksTab.classList.remove('active');
        emailList.style.display = 'block';
        webhookList.style.display = 'none';
    } else if (tab === 'webhooks') {
        webhooksTab.classList.add('active');
        inboxTab.classList.remove('active');
        emailList.style.display = 'none';
        webhookList.style.display = 'block';
    }
}

// Load webhooks for current address
async function loadWebhooks(address) {
    if (!address) return;
    
    try {
        const passwordParam = mailboxPassword ? `?password=${encodeURIComponent(mailboxPassword)}` : '';
        const response = await authFetch(`/api/webhooks/${encodeURIComponent(address)}${passwordParam}`);
        const data = await response.json();
        
        webhooks = data.webhooks || [];
        displayWebhooks(webhooks);
    } catch (error) {
        console.error('Failed to load webhooks:', error);
        webhookList.innerHTML = '<div class="empty-state"><p>❌ Failed to load webhooks</p></div>';
    }
}

// Display webhooks
function displayWebhooks(webhooksToDisplay) {
    if (webhooksToDisplay.length === 0) {
        webhookList.innerHTML = `
            <div class="empty-state">
                <p>🔗 No webhooks configured for this mailbox</p>
                <button class="btn-primary" onclick="showAddWebhookForm()">Add Webhook</button>
            </div>
        `;
        return;
    }
    
    webhookList.innerHTML = `
        <div class="webhook-header">
            <h3>Webhooks (${webhooksToDisplay.length})</h3>
            <button class="btn-primary" onclick="showAddWebhookForm()">Add Webhook</button>
        </div>
        <div class="webhook-items">
            ${webhooksToDisplay.map(webhook => `
                <div class="webhook-item">
                    <div class="webhook-info">
                        <div class="webhook-url">${escapeHtml(webhook.webhook_url)}</div>
                        <div class="webhook-events">
                            Events: ${webhook.events.map(e => e.charAt(0).toUpperCase() + e.slice(1)).join(', ')}
                        </div>
                        <div class="webhook-status">
                            Status: <span class="status-${webhook.enabled ? 'enabled' : 'disabled'}">${webhook.enabled ? 'Enabled' : 'Disabled'}</span>
                        </div>
                    </div>
                    <div class="webhook-actions">
                        <button class="btn-secondary" onclick="testWebhook('${webhook.id}')">Test</button>
                        <button class="btn-secondary" onclick="editWebhook('${webhook.id}')">Edit</button>
                        <button class="btn-danger" onclick="deleteWebhook('${webhook.id}')">Delete</button>
                    </div>
                </div>
            `).join('')}
        </div>
    `;
}

// Show add webhook form
function showAddWebhookForm() {
    const formHtml = `
        <div class="webhook-form">
            <h3>Add New Webhook</h3>
            <form id="addWebhookForm">
                <div class="form-group">
                    <label for="webhookUrl">Webhook URL:</label>
                    <input type="url" id="webhookUrl" placeholder="http://localhost:3009 or https://example.com/webhook" required>
                    <small style="color: var(--text-secondary); font-size: 0.875rem; margin-top: 0.25rem; display: block;">
                        Enter the full URL including protocol (http:// or https://). For local testing, use http://localhost:PORT
                    </small>
                </div>
                <div class="form-group">
                    <label>Events to trigger:</label>
                    <div class="checkbox-group">
                        <label><input type="checkbox" name="events" value="arrival" checked> Email Arrival</label>
                        <label><input type="checkbox" name="events" value="deletion"> Email Deletion</label>
                    </div>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Create Webhook</button>
                    <button type="button" class="btn-secondary" onclick="cancelAddWebhook()">Cancel</button>
                </div>
            </form>
        </div>
    `;
    
    webhookList.innerHTML = formHtml;
    
    document.getElementById('addWebhookForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await createWebhook();
    });
}

// Create webhook
async function createWebhook() {
    const url = document.getElementById('webhookUrl').value;
    const eventCheckboxes = document.querySelectorAll('input[name="events"]:checked');
    const events = Array.from(eventCheckboxes).map(cb => cb.value);
    
    if (events.length === 0) {
        alert('Please select at least one event');
        return;
    }
    
    try {
        const requestBody = {
            mailbox_address: currentAddress,
            webhook_url: url,
            events: events
        };
        
        // Include password if mailbox is protected
        if (mailboxPassword) {
            requestBody.password = mailboxPassword;
        }
        
        const response = await authFetch('/api/webhooks', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(requestBody)
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            const error = await response.text();
            alert('Failed to create webhook: ' + error);
        }
    } catch (error) {
        console.error('Failed to create webhook:', error);
        alert('Failed to create webhook');
    }
}

// Test webhook
async function testWebhook(webhookId) {
    try {
        const response = await authFetch(`/api/webhook/${webhookId}/test`, {
            method: 'POST'
        });
        
        const result = await response.json();
        if (result.success) {
            alert('Webhook test successful!');
        } else {
            alert('Webhook test failed');
        }
    } catch (error) {
        console.error('Failed to test webhook:', error);
        alert('Failed to test webhook');
    }
}

// Edit webhook
function editWebhook(webhookId) {
    const webhook = webhooks.find(w => w.id === webhookId);
    if (!webhook) return;
    
    const formHtml = `
        <div class="webhook-form">
            <h3>Edit Webhook</h3>
            <form id="editWebhookForm">
                <div class="form-group">
                    <label for="editWebhookUrl">Webhook URL:</label>
                    <input type="url" id="editWebhookUrl" value="${escapeHtml(webhook.webhook_url)}" required>
                    <small style="color: var(--text-secondary); font-size: 0.875rem; margin-top: 0.25rem; display: block;">
                        Enter the full URL including protocol (http:// or https://). For local testing, use http://localhost:PORT
                    </small>
                </div>
                <div class="form-group">
                    <label>Events to trigger:</label>
                    <div class="checkbox-group">
                        <label><input type="checkbox" name="editEvents" value="arrival" ${webhook.events.includes('arrival') ? 'checked' : ''}> Email Arrival</label>
                        <label><input type="checkbox" name="editEvents" value="deletion" ${webhook.events.includes('deletion') ? 'checked' : ''}> Email Deletion</label>
                    </div>
                </div>
                <div class="form-group">
                    <label>
                        <input type="checkbox" name="enabled" ${webhook.enabled ? 'checked' : ''}> Enabled
                    </label>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Update Webhook</button>
                    <button type="button" class="btn-secondary" onclick="cancelEditWebhook()">Cancel</button>
                </div>
            </form>
        </div>
    `;
    
    webhookList.innerHTML = formHtml;
    
    document.getElementById('editWebhookForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await updateWebhook(webhookId);
    });
}

// Update webhook
async function updateWebhook(webhookId) {
    const url = document.getElementById('editWebhookUrl').value;
    const eventCheckboxes = document.querySelectorAll('input[name="editEvents"]:checked');
    const events = Array.from(eventCheckboxes).map(cb => cb.value);
    const enabled = document.querySelector('input[name="enabled"]').checked;
    
    if (events.length === 0) {
        alert('Please select at least one event');
        return;
    }
    
    try {
        const response = await authFetch(`/api/webhook/${webhookId}`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                webhook_url: url,
                events: events,
                enabled: enabled
            })
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            const error = await response.text();
            alert('Failed to update webhook: ' + error);
        }
    } catch (error) {
        console.error('Failed to update webhook:', error);
        alert('Failed to update webhook');
    }
}

// Delete webhook
async function deleteWebhook(webhookId) {
    if (!confirm('Are you sure you want to delete this webhook?')) {
        return;
    }
    
    try {
        const response = await authFetch(`/api/webhook/${webhookId}`, {
            method: 'DELETE'
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            alert('Failed to delete webhook');
        }
    } catch (error) {
        console.error('Failed to delete webhook:', error);
        alert('Failed to delete webhook');
    }
}

// Cancel add webhook
function cancelAddWebhook() {
    displayWebhooks(webhooks);
}

// Cancel edit webhook
function cancelEditWebhook() {
    displayWebhooks(webhooks);
}

