'use strict';

const $ = (id) => document.getElementById(id);
const esc = (t) => String(t ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#039;' }[c]));
const enc = encodeURIComponent;
const M = 6e4, H = 36e5, D = 864e5;
const MOBILE = window.matchMedia('(max-width: 759px)');

const S = {
    domain: '', authEnabled: false, outbound: false, authDomains: null, registrationOpen: false,
    token: localStorage.getItem('auth_token'), user: null,
    address: '', password: '', claimed: false, adminMode: false,
    view: 'inbox', emails: [], sel: null, results: null, query: '',
    webhooks: [], editingHook: null,
    adminInboxes: [], adminTotal: 0,
    modal: null, pending: '', connected: false, loading: false,
};

// ---------------------------------------------------------------- helpers

function randomLocal() {
    const a = ['quiet', 'amber', 'brisk', 'lunar', 'mossy', 'spare', 'tidal', 'rusty', 'woven', 'polar'];
    const n = ['otter', 'heron', 'finch', 'maple', 'cedar', 'comet', 'pike', 'wren', 'lynx', 'fern'];
    const p = (x) => x[Math.floor(Math.random() * x.length)];
    return `${p(a)}-${p(n)}-${1000 + Math.floor(Math.random() * 9000)}`;
}

function normalize(raw) {
    let a = (raw || '').trim().toLowerCase();
    if (a && !a.includes('@') && S.domain) a += '@' + S.domain;
    return a;
}

// Addresses on the server's own domain are shown as just the name.
const localPart = (a) => (S.domain && a.endsWith('@' + S.domain) ? a.slice(0, -S.domain.length - 1) : a);

function rel(ts) {
    const t = new Date(ts).getTime(), d = Date.now() - t;
    if (d < M) return 'Just now';
    if (d < H) return Math.floor(d / M) + ' min';
    if (d < D) return Math.floor(d / H) + ' h';
    if (d < 2 * D) return 'Yesterday';
    return new Date(t).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

const fullDate = (ts) => new Date(ts).toLocaleString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric', hour: '2-digit', minute: '2-digit' });

function fileSize(b) {
    if (!b) return '0 B';
    const i = Math.min(3, Math.floor(Math.log(b) / Math.log(1024)));
    return Math.round(b / 1024 ** i * 10) / 10 + ' ' + ['B', 'KB', 'MB', 'GB'][i];
}

function splitFrom(from) {
    const m = /^\s*"?([^"<]*?)"?\s*<([^>]+)>/.exec(from || '');
    return m && m[1] ? { name: m[1], addr: m[2] } : { name: from || '', addr: (m && m[2]) || from || '' };
}

// Real tags only: plain text often contains "<user@host>" addresses.
const isHtml = (body) => /<\/?(html|head|body|div|p|br|table|tr|td|span|a|img|style|font|b|i|strong|em|ul|ol|li|h[1-6])\b[^>]*>/i.test(body || '');

function plain(e) {
    if (e._plain == null) {
        let text = e.body;
        if (isHtml(text)) {
            const doc = new DOMParser().parseFromString(text.replace(/</g, ' <'), 'text/html');
            doc.querySelectorAll('script, style, head').forEach((n) => n.remove());
            text = doc.body.textContent;
        }
        e._plain = (text || '').replace(/\s+/g, ' ').trim();
    }
    return e._plain;
}

// HTML mail often opens with a preheader or heading that restates the subject.
function preview(e) {
    const t = plain(e), sub = (e.subject || '').trim();
    return (sub && t.toLowerCase().startsWith(sub.toLowerCase()) ? t.slice(sub.length).replace(/^[\s:|–—-]+/, '') : t).slice(0, 160);
}

// FTS snippets are raw body text wrapped in <mark>; escape everything else.
const safeSnippet = (s) => esc(s).replace(/&lt;(\/?)mark&gt;/g, '<$1mark>');

let toastTimer;
function toast(msg) {
    $('toastText').textContent = msg;
    $('toast').hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { $('toast').hidden = true; }, 4000);
}

// ---------------------------------------------------------------- API

async function api(url, opts = {}) {
    opts.headers = { ...(opts.headers || {}) };
    if (opts.json !== undefined) {
        opts.headers['Content-Type'] = 'application/json';
        opts.body = JSON.stringify(opts.json);
    }
    if (S.authEnabled && S.token) opts.headers.Authorization = `Bearer ${S.token}`;
    const res = await fetch(url, opts);
    if (res.status === 401 && S.authEnabled) {
        const text = await res.clone().text();
        // Mailbox-password 401s mention "password"; anything else is an expired session.
        if (!/password/i.test(text)) {
            logout();
            throw new Error('Please log in again.');
        }
    }
    return res;
}

async function apiJson(url, opts) {
    const res = await api(url, opts);
    if (!res.ok) throw new Error((await res.text()) || `HTTP ${res.status}`);
    return res.json();
}

const pwParam = (pre = '?') => (S.password ? `${pre}password=${enc(S.password)}` : '');

// ---------------------------------------------------------------- auth

async function initAuth() {
    try {
        const [status, cfg] = await Promise.all([
            fetch('/api/auth/status').then((r) => r.json()),
            fetch('/api/config').then((r) => r.json()).catch(() => ({})),
        ]);
        S.domain = cfg.domain_name || '';
        S.authEnabled = !!status.auth_enabled;
        S.outbound = !!status.outbound_enabled;
        S.authDomains = status.auth_domains || null;
        S.registrationOpen = !!status.registration_open;
    } catch (e) {
        console.error('Failed to load server status', e);
    }
    if (S.authEnabled && S.token) {
        try {
            const r = await fetch('/api/auth/me', { headers: { Authorization: `Bearer ${S.token}` } });
            if (r.ok) S.user = await r.json();
        } catch (e) { /* treated as logged out */ }
        if (!S.user) { S.token = null; localStorage.removeItem('auth_token'); }
    }
}

const needsLogin = () => S.authEnabled && !S.user;

function logout() {
    S.token = null;
    S.user = null;
    localStorage.removeItem('auth_token');
    closeMailbox();
    if (S.view === 'admin') S.view = 'inbox';
    render();
    if (S.authEnabled) openModal(S.registrationOpen ? 'register' : 'login');
}

async function submitAuth(kind) {
    const email = $('mEmail').value.trim(), password = $('mPw').value;
    if (!email || !password) return modalError('Enter your email and password.');
    if (kind === 'register') {
        if (password.length < 8) return modalError('Password must be at least 8 characters.');
        if (password !== $('mPw2').value) return modalError('Passwords do not match.');
    }
    const res = await fetch(`/api/auth/${kind}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ email, password }) });
    if (!res.ok) return modalError((await res.text()) || (kind === 'login' ? 'Login failed.' : 'Registration failed.'));
    const data = await res.json();
    S.token = data.token;
    S.user = data.user;
    localStorage.setItem('auth_token', S.token);
    closeModal();
    toast(kind === 'login' ? `Logged in as ${S.user.email}` : 'Account created.');
    render();
    loadFromUrl();
}

// ---------------------------------------------------------------- mailbox

const storedPw = (a) => localStorage.getItem(`mailbox_password_${a}`) || '';
const storePw = (a, p) => localStorage.setItem(`mailbox_password_${a}`, p);
const clearPw = (a) => localStorage.removeItem(`mailbox_password_${a}`);

function recents() {
    try { return JSON.parse(localStorage.getItem('recent_mailboxes')) || []; } catch { return []; }
}
function pushRecent(a) {
    localStorage.setItem('recent_mailboxes', JSON.stringify([a, ...recents().filter((r) => r !== a)].slice(0, 5)));
}

function setUrl(address) {
    const url = new URL(location);
    const current = normalize(url.searchParams.get('mailbox') || '');
    if (address) url.searchParams.set('mailbox', address); else url.searchParams.delete('mailbox');
    // Same mailbox (initial load, back/forward) only canonicalises the URL.
    if (current === address) history.replaceState({}, '', url); else history.pushState({}, '', url);
}

function closeMailbox() {
    stopSocket();
    Object.assign(S, { address: '', password: '', claimed: false, adminMode: false, emails: [], sel: null, results: null, query: '', webhooks: [], editingHook: null, loading: false });
    $('searchInput').value = '';
    updateFavicon();
}

async function openMailbox(raw) {
    if (needsLogin()) return openModal('login');
    const address = normalize(raw);
    if (!address) return;
    closeDrawer();
    let locked = false;
    try {
        locked = !!(await apiJson(`/api/mailbox/${enc(address)}/status`)).is_locked;
    } catch (e) {
        return toast(e.message);
    }
    const pw = storedPw(address);
    if (locked) {
        if (pw && (await api(`/api/emails/${enc(address)}?password=${enc(pw)}`)).ok) return loadMailbox(address, pw, true);
        clearPw(address);
        S.pending = address;
        return openModal('unlock');
    }
    loadMailbox(address, '', false);
}

async function loadMailbox(address, password, claimed) {
    closeMailbox();
    Object.assign(S, { address, password, claimed, view: S.view === 'admin' ? 'inbox' : S.view });
    $('openInput').value = localPart(address);
    setUrl(address);
    pushRecent(address);
    S.loading = true;
    render();
    let emails = [];
    try {
        emails = (await apiJson(`/api/emails/${enc(address)}${pwParam()}`)).emails || [];
    } catch (e) {
        toast(`Failed to load emails: ${e.message}`);
    }
    if (S.address !== address || S.adminMode) return; // another mailbox was opened meanwhile
    S.loading = false;
    S.emails = emails;
    if (!MOBILE.matches && S.emails[0]) S.sel = S.emails[0].id;
    render();
    connectSocket(address);
    loadWebhooks();
}

async function submitUnlock() {
    const pw = $('mPw').value;
    if (!pw) return modalError('Enter the mailbox password.');
    const res = await api(`/api/emails/${enc(S.pending)}?password=${enc(pw)}`);
    if (res.status === 401) return modalError('Incorrect password.');
    if (!res.ok) return modalError('Failed to unlock mailbox.');
    storePw(S.pending, pw);
    closeModal();
    loadMailbox(S.pending, pw, true);
}

async function submitClaim() {
    const pw = $('mPw').value;
    if (!pw) return modalError('Enter a password.');
    if (pw !== $('mPw2').value) return modalError('Passwords do not match.');
    const res = await api(`/api/mailbox/${enc(S.address)}/claim`, { method: 'POST', json: { password: pw } });
    if (!res.ok) return modalError((await res.text()) || 'Failed to claim mailbox.');
    storePw(S.address, pw);
    S.password = pw;
    S.claimed = true;
    closeModal();
    render();
    toast('Mailbox claimed. The same password works for IMAP.');
}

async function submitRelease() {
    const res = await api(`/api/mailbox/${enc(S.address)}/release`, { method: 'POST', json: { password: S.password } });
    if (!res.ok) return modalError((await res.text()) || 'Failed to release mailbox.');
    clearPw(S.address);
    S.password = '';
    S.claimed = false;
    closeModal();
    render();
    toast('Mailbox released. Anyone can open or claim it now.');
}

// ---------------------------------------------------------------- emails

function selectEmail(id) {
    S.sel = id;
    const e = S.emails.find((x) => x.id === id);
    if (e) e.isNew = false;
    document.querySelector('.reader .scroll')?.scrollTo(0, 0);
    render();
    updateFavicon();
}

async function openSearchResult(id) {
    if (!S.emails.some((e) => e.id === id)) {
        try {
            S.emails.push(await apiJson(`/api/email/${enc(id)}${pwParam()}`));
        } catch (e) {
            return toast(`Failed to load email: ${e.message}`);
        }
    }
    selectEmail(id);
}

async function submitDelete() {
    const id = S.sel;
    const res = await api(`/api/email/${enc(id)}${pwParam()}`, { method: 'DELETE' });
    if (!res.ok) return modalError(`Failed to delete email (HTTP ${res.status}).`);
    removeEmail(id);
    closeModal();
    toast('Email deleted.');
}

function removeEmail(id) {
    S.emails = S.emails.filter((e) => e.id !== id);
    if (S.results) S.results = S.results.filter((r) => r.id !== id);
    if (S.sel === id) S.sel = !MOBILE.matches && S.emails[0] ? S.emails[0].id : null;
    render();
    updateFavicon();
}

async function runSearch() {
    const q = $('searchInput').value.trim();
    S.query = q;
    S.sel = null;
    if (!q) { S.results = null; return render(); }
    try {
        const data = await apiJson(`/api/search?q=${enc(q)}&mailbox=${enc(S.address)}${pwParam('&')}`);
        S.results = data.results || [];
    } catch (e) {
        S.results = [];
        toast(`Search failed: ${e.message}`);
    }
    render();
}

function download(att) {
    const bin = atob(att.content);
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    const url = URL.createObjectURL(new Blob([bytes], { type: att.content_type }));
    const a = Object.assign(document.createElement('a'), { href: url, download: att.filename });
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
}

async function submitCompose() {
    const to = $('cTo').value.trim(), subject = $('cSubj').value.trim();
    if (!to.includes('@')) return modalError('Enter a recipient address.');
    if (!subject) return modalError('Enter a subject.');
    $('mOk').disabled = true;
    try {
        const res = await api('/api/send', { method: 'POST', json: { to, subject, body_text: $('cBody').value, from_address: S.address.split('@')[0] } });
        if (!res.ok) return modalError((await res.text()) || 'Failed to send email.');
        closeModal();
        toast(`Sent to ${to}`);
    } catch (e) {
        modalError(e.message);
    } finally {
        $('mOk').disabled = false;
    }
}

function openCompose(reply) {
    openModal('compose');
    $('cTo').value = reply ? splitFrom(reply.from).addr : '';
    $('cSubj').value = reply ? (/^re:/i.test(reply.subject) ? reply.subject : `Re: ${reply.subject}`) : '';
    $('cBody').value = reply && !isHtml(reply.body)
        ? `\n\n---\nOn ${new Date(reply.timestamp).toLocaleString()}, ${reply.from} wrote:\n\n${reply.body.split('\n').map((l) => '> ' + l).join('\n')}`
        : '';
    $('mTitle').textContent = reply ? 'Reply' : 'New message';
    (reply ? $('cBody') : $('cTo')).focus();
    if (reply) $('cBody').setSelectionRange(0, 0);
}

// ---------------------------------------------------------------- admin

async function showAdmin() {
    closeMailbox();
    setUrl('');
    S.view = 'admin';
    S.adminInboxes = [];
    S.adminTotal = 0;
    closeDrawer();
    render();
    loadMoreAdmin();
}

async function loadMoreAdmin() {
    try {
        const { inboxes, total } = await apiJson(`/api/admin/inboxes?limit=100&offset=${S.adminInboxes.length}`);
        S.adminInboxes = S.adminInboxes.concat(inboxes);
        S.adminTotal = total;
    } catch (e) {
        toast(`Failed to load inboxes: ${e.message}`);
    }
    render();
}

// Uses the admin endpoint only: the regular per-mailbox endpoints would 401 on
// password-protected inboxes, which the api() wrapper treats as session expiry.
async function openAdminInbox(address) {
    closeMailbox();
    Object.assign(S, { address, adminMode: true, view: 'inbox', loading: true });
    render();
    let emails = [];
    try {
        emails = (await apiJson(`/api/admin/inboxes/${enc(address)}/emails`)).emails || [];
    } catch (e) {
        toast(`Failed to load emails: ${e.message}`);
    }
    if (S.address !== address || !S.adminMode) return;
    S.loading = false;
    S.emails = emails;
    if (!MOBILE.matches && S.emails[0]) S.sel = S.emails[0].id;
    render();
}

// ---------------------------------------------------------------- webhooks

async function loadWebhooks() {
    if (!S.address || S.adminMode) return;
    try {
        S.webhooks = (await apiJson(`/api/webhooks/${enc(S.address)}${pwParam()}`)).webhooks || [];
    } catch (e) {
        console.error('Failed to load webhooks', e);
    }
    render();
}

function openHookForm(hook) {
    S.editingHook = hook || null;
    $('hookFormTitle').textContent = hook ? 'Edit webhook' : 'New webhook';
    $('hookUrl').value = hook ? hook.webhook_url : '';
    document.querySelectorAll('#hookForm [name=ev]').forEach((c) => { c.checked = hook ? hook.events.includes(c.value) : c.value === 'arrival'; });
    $('hookErr').hidden = true;
    $('hookForm').hidden = false;
    $('addHookBtn').hidden = true;
    $('hookUrl').focus();
}

function closeHookForm() {
    $('hookForm').hidden = true;
    $('addHookBtn').hidden = false;
}

async function saveHook(ev) {
    ev.preventDefault();
    const url = $('hookUrl').value.trim();
    const events = [...document.querySelectorAll('#hookForm [name=ev]:checked')].map((c) => c.value);
    const fail = (m) => { $('hookErr').textContent = m; $('hookErr').hidden = false; };
    if (!/^https?:\/\/.+/.test(url)) return fail('Enter a full URL starting with http:// or https://.');
    if (!events.length) return fail('Pick at least one event.');
    const h = S.editingHook;
    const res = h
        ? await api(`/api/webhook/${enc(h.id)}`, { method: 'PUT', json: { webhook_url: url, events, enabled: h.enabled } })
        : await api('/api/webhooks', { method: 'POST', json: { mailbox_address: S.address, webhook_url: url, events, ...(S.password && { password: S.password }) } });
    if (!res.ok) return fail((await res.text()) || 'Failed to save webhook.');
    closeHookForm();
    toast(h ? 'Webhook updated.' : 'Webhook added.');
    loadWebhooks();
}

async function hookAction(id, action) {
    const h = S.webhooks.find((w) => w.id === id);
    if (!h) return;
    try {
        if (action === 'test') {
            const { success } = await apiJson(`/api/webhook/${enc(id)}/test`, { method: 'POST' });
            return toast(success ? 'Test payload delivered.' : 'Test delivery failed.');
        }
        if (action === 'edit') return openHookForm(h);
        if (action === 'toggle') {
            await apiJson(`/api/webhook/${enc(id)}`, { method: 'PUT', json: { webhook_url: h.webhook_url, events: h.events, enabled: !h.enabled } });
        } else if (action === 'delete') {
            const res = await api(`/api/webhook/${enc(id)}`, { method: 'DELETE' });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            toast('Webhook deleted.');
        }
        loadWebhooks();
    } catch (e) {
        toast(`Webhook ${action} failed: ${e.message}`);
    }
}

// ---------------------------------------------------------------- websocket

let ws = null, wsRetry = 0, wsTimer = null, wsWanted = false;

function connectSocket(address) {
    clearTimeout(wsTimer);
    wsWanted = true;
    if (ws) { const old = ws; ws = null; old.close(); }
    const sock = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/api/ws/${enc(address)}`);
    ws = sock;
    sock.onopen = () => { wsRetry = 0; S.connected = true; render(); };
    sock.onmessage = (event) => {
        if (ws !== sock) return;
        let data;
        try { data = JSON.parse(event.data); } catch { return; }
        if (data.type === 'Email') {
            data.isNew = true;
            S.emails.unshift(data);
            render();
            updateFavicon();
            toast(`New email from ${splitFrom(data.from).name || data.from}`);
            notify(data);
        } else if (data.type === 'EmailDeleted' && S.emails.some((e) => e.id === data.id)) {
            removeEmail(data.id);
        }
    };
    sock.onclose = () => {
        if (ws !== sock) return;
        S.connected = false;
        render();
        if (wsWanted) wsTimer = setTimeout(() => connectSocket(address), Math.min(1000 * 2 ** wsRetry++, 30000));
    };
}

function stopSocket() {
    wsWanted = false;
    clearTimeout(wsTimer);
    wsRetry = 0;
    if (ws) { const s = ws; ws = null; s.close(); }
    S.connected = false;
}

async function resync() {
    const address = S.address;
    if (!address || S.adminMode || S.loading || (ws && ws.readyState !== WebSocket.CLOSED)) return;
    connectSocket(address);
    try {
        const fresh = (await apiJson(`/api/emails/${enc(address)}${pwParam()}`)).emails || [];
        if (S.address !== address || S.adminMode) return;
        const unread = new Set(S.emails.filter((e) => e.isNew).map((e) => e.id));
        S.emails = fresh.map((e) => ({ ...e, isNew: unread.has(e.id) }));
        render();
    } catch (e) { /* next reconnect will retry */ }
}

function notify(email) {
    if (!('Notification' in window) || Notification.permission !== 'granted' || !document.hidden) return;
    const n = new Notification('New email', { body: `From: ${email.from}\nSubject: ${email.subject}`, icon: '/logo.svg', tag: 'new-email' });
    n.onclick = () => { window.focus(); selectEmail(email.id); n.close(); };
}

let baseFavicon = null;
function updateFavicon() {
    const link = document.querySelector('link[rel="icon"]');
    baseFavicon ??= link.href;
    if (!S.emails.some((e) => e.isNew)) { link.href = baseFavicon; return; }
    const img = new Image();
    img.onload = () => {
        const c = Object.assign(document.createElement('canvas'), { width: 32, height: 32 });
        const ctx = c.getContext('2d');
        ctx.drawImage(img, 0, 0, 32, 32);
        ctx.globalCompositeOperation = 'source-atop';
        ctx.fillStyle = '#d0302f';
        ctx.fillRect(0, 0, 32, 32);
        link.href = c.toDataURL('image/png');
    };
    img.src = baseFavicon;
}

// ---------------------------------------------------------------- modal

const MODALS = {
    claim: () => ['Claim this mailbox', `Nobody has claimed ${S.address} yet. Set a password so only you can read it. You can release it later.`, 'Claim mailbox', 'Skip'],
    unlock: () => ['Mailbox protected', `${S.pending} is password protected. Enter the password to open it.`, 'Unlock', 'Cancel'],
    release: () => ['Release this mailbox?', 'It will become publicly accessible and anyone could claim it.', 'Release', 'Cancel'],
    delete: () => ['Delete this email?', 'This cannot be undone.', 'Delete', 'Cancel'],
    login: () => ['Log in', 'This server requires an account to read mail.', 'Log in', 'Cancel'],
    register: () => ['Create an account', S.authDomains ? `Registration is limited to addresses at ${S.authDomains.join(', ')}.` : 'Create an account to read mail on this server.', 'Register', 'Cancel'],
    compose: () => ['New message', 'Sent from this mailbox through the server’s outbound relay.', 'Send', 'Discard'],
};

function openModal(kind) {
    const m = kind, [title, desc, ok, cancel] = MODALS[m]();
    S.modal = m;
    const dlg = $('modal');
    dlg.classList.toggle('wide', m === 'compose');
    $('mTitle').textContent = title;
    $('mDesc').textContent = desc;
    $('mOk').textContent = ok;
    $('mOk').className = `btn ${m === 'release' || m === 'delete' ? 'danger' : 'primary'}`;
    $('mCancel').textContent = cancel;
    const auth = m === 'login' || m === 'register';
    $('mCancel').hidden = auth && needsLogin();
    $('fEmail').hidden = !auth;
    $('fPw').hidden = !['claim', 'unlock', 'login', 'register'].includes(m);
    $('fPw2').hidden = !(m === 'claim' || m === 'register');
    $('fCompose').hidden = m !== 'compose';
    $('toRegister').hidden = m !== 'login';
    $('toLogin').hidden = m !== 'register';
    $('mEmail').placeholder = m === 'register' && S.authDomains ? `you@${S.authDomains[0]}` : 'you@example.com';
    $('mPw').placeholder = m === 'register' ? 'At least 8 characters' : '';
    $('mPw').autocomplete = auth ? (m === 'login' ? 'current-password' : 'new-password') : 'off';
    $('pwHelp').textContent = m === 'claim' ? 'Also used as the IMAP password for this mailbox.' : '';
    $('cFrom').textContent = S.address;
    $('mPw').value = $('mPw2').value = '';
    $('mErr').hidden = true;
    closeDrawer();
    if (!dlg.open) dlg.showModal();
    const first = [$('mEmail'), $('mPw')].find((el) => !el.closest('[hidden]'));
    (first || $('mOk')).focus();
}

function closeModal() {
    S.modal = null;
    if ($('modal').open) $('modal').close();
}

function modalError(msg) {
    $('mErr').textContent = msg;
    $('mErr').hidden = false;
}

async function submitModal() {
    const fn = { claim: submitClaim, unlock: submitUnlock, release: submitRelease, delete: submitDelete, compose: submitCompose,
        login: () => submitAuth('login'), register: () => submitAuth('register') }[S.modal];
    try { await fn?.(); } catch (e) { modalError(e.message); }
}

// ---------------------------------------------------------------- render

function closeDrawer() { $('app').classList.remove('drawer'); $('backdrop').hidden = true; }

function setView(v) {
    S.view = v;
    closeHookForm();
    closeDrawer();
    render();
}

function render() {
    const has = !!S.address, admin = !!S.user?.is_admin;
    const adminList = S.view === 'admin' && !has;
    const inbox = S.view === 'inbox' && has;
    const searching = inbox && !!S.query;
    const hooks = S.view === 'webhooks' && has && !S.adminMode;
    const unread = S.emails.filter((e) => e.isNew).length;
    const canCompose = S.outbound && has && !S.adminMode && !needsLogin();

    // Sidebar
    $('currentBox').hidden = !has;
    $('addr').textContent = S.address;
    const lb = $('lockBadge');
    lb.textContent = S.adminMode ? 'Admin access' : S.claimed ? 'Claimed by you' : 'Unclaimed';
    lb.className = `badge ${S.adminMode ? 'warn' : S.claimed ? 'ok' : ''}`;
    $('claimBtn').textContent = S.adminMode ? '' : S.claimed ? 'Release' : 'Claim with a password';
    $('composeBtn').hidden = $('composeBtnM').hidden = !canCompose;
    $('userBox').hidden = !S.user;
    $('userEmail').textContent = S.user?.email || '';
    $('userRole').textContent = admin ? 'Admin' : 'Signed in';
    $('loginBtn').hidden = !S.authEnabled || !!S.user;
    document.querySelectorAll('.domain').forEach((el) => { el.textContent = S.domain || 'this server'; });

    const navs = [
        { key: 'inbox', label: 'Inbox', count: has ? (unread ? `${unread} new` : String(S.emails.length)) : '', on: has },
        { key: 'webhooks', label: 'Webhooks', count: has && !S.adminMode ? String(S.webhooks.length) : '', on: has && !S.adminMode },
    ];
    if (admin) navs.push({ key: 'admin', label: 'All inboxes', count: S.adminTotal ? String(S.adminTotal) : '', on: true });
    const active = (n) => n.on && (n.key === 'admin' ? adminList : S.view === n.key && has);
    $('nav').innerHTML = navs.map((n) => `<button data-view="${n.key}" class="${active(n) ? 'active' : ''}" ${n.on ? '' : 'disabled'}><span>${n.label}</span><span class="count">${esc(n.count)}</span></button>`).join('');
    const tabs = navs.filter((n) => n.on);
    $('tabs').hidden = !tabs.length;
    $('tabs').innerHTML = tabs.map((n) => `<button data-view="${n.key}" class="${active(n) ? 'active' : ''}">${n.label}</button>`).join('');
    $('mobileTitle').textContent = has ? S.address : adminList ? 'All inboxes' : 'dyn-ip email';

    // Main panes
    const showEmpty = !has && !adminList;
    $('emptyView').hidden = !showEmpty;
    $('hooksView').hidden = !hooks;
    $('listPane').hidden = $('readerPane').hidden = showEmpty || hooks;
    if (showEmpty) renderRecents();
    if (hooks) renderHooks();
    if (!showEmpty && !hooks) renderList(adminList, searching, inbox);

    const selEmail = !adminList && S.emails.find((e) => e.id === S.sel);
    $('app').classList.toggle('detail', !!selEmail);
    renderReader(selEmail, adminList);
}

function renderRecents() {
    const list = recents();
    $('recents').hidden = !list.length;
    $('recents').innerHTML = '<div class="head">Recently opened on this device</div>' +
        list.map((a) => `<button data-open="${esc(a)}"><span>${esc(a)}</span></button>`).join('');
}

function renderList(adminList, searching, inbox) {
    $('adminBanner').hidden = !S.adminMode;
    $('searchBox').hidden = !inbox || S.adminMode;
    $('liveBadge').hidden = !S.address || S.adminMode || S.loading;
    $('liveBadge').className = `badge ${S.connected ? 'ok' : ''}`;
    $('liveText').textContent = S.connected ? 'Live' : 'Reconnecting';

    let rows = '', title = 'Inbox', count = '', empty = '';
    if (adminList) {
        title = 'All inboxes';
        count = `${S.adminTotal} inboxes`;
        empty = 'No inboxes yet.';
        rows = S.adminInboxes.map((i) => `
            <div class="mrow inbox" data-admin="${esc(i.address)}">
                <div class="l1"><span class="dot"></span><span class="top">${esc(i.address)}</span><span class="time">${rel(i.last_received)}</span></div>
                <div class="l2"><span class="sub">${i.email_count} email${i.email_count === 1 ? '' : 's'}</span></div>
            </div>`).join('');
        if (S.adminInboxes.length < S.adminTotal) rows += `<button class="btn more" id="adminMore">Load more (${S.adminInboxes.length} of ${S.adminTotal})</button>`;
    } else if (searching) {
        title = 'Search';
        const r = S.results;
        count = r ? `${r.length} result${r.length === 1 ? '' : 's'}` : '';
        empty = `Nothing matches “${esc(S.query)}”.`;
        rows = (r || []).map((x) => `
            <div class="mrow ${x.id === S.sel ? 'active' : ''}" data-result="${esc(x.id)}">
                <div class="l1"><span class="dot"></span><span class="top">${esc(splitFrom(x.from).name)}</span><span class="time">${rel(x.timestamp)}</span></div>
                <div class="l2"><span class="sub">${esc(x.subject)}</span></div>
                <div class="pre">${safeSnippet(x.snippet)}</div>
            </div>`).join('');
    } else {
        count = `${S.emails.length} email${S.emails.length === 1 ? '' : 's'}`;
        empty = S.loading ? 'Loading…' : 'No mail yet. It will appear here as soon as it arrives.';
        rows = S.emails.map((e) => {
            const n = e.attachments?.length || 0;
            return `
            <div class="mrow ${e.isNew ? 'new' : ''} ${e.id === S.sel ? 'active' : ''}" data-email="${esc(e.id)}">
                <div class="l1"><span class="dot"></span><span class="top">${esc(splitFrom(e.from).name)}</span><span class="time">${rel(e.timestamp)}</span></div>
                <div class="l2"><span class="sub">${esc(e.subject || '(no subject)')}</span>${n ? `<span class="tag">${n} file${n === 1 ? '' : 's'}</span>` : ''}</div>
                <div class="pre">${esc(preview(e))}</div>
            </div>`;
        }).join('');
    }
    $('listTitle').textContent = title;
    $('listCount').textContent = count;
    $('rows').innerHTML = rows || `<div class="empty">${empty}</div>`;
}

let renderedEmailId = null;
function renderReader(e, adminList) {
    $('readerSel').hidden = !e;
    $('readerEmpty').hidden = !!e;
    if (!e) {
        renderedEmailId = null;
        let title = 'Select an email to read it', text = 'Messages open here. New mail is pushed over a live connection, so there is no need to refresh.', swaks = '';
        if (adminList) { title = 'Pick an inbox'; text = 'As an admin you can open any inbox, including password-protected ones.'; }
        else if (S.address && !S.loading && !S.emails.length && !S.query) {
            title = 'This inbox is empty';
            text = `Send something to ${S.address} and it will show up straight away. To test from a terminal:`;
            swaks = `swaks --to ${S.address} \\\n      --server ${S.domain || location.hostname}:25 \\\n      --body "Hello from temporary mail!"`;
        }
        $('reTitle').textContent = title;
        $('reText').textContent = text;
        $('swaks').hidden = !swaks;
        $('swaks').textContent = swaks;
        return;
    }
    $('replyBtn').hidden = !S.outbound || S.adminMode;
    $('deleteBtn').hidden = S.adminMode;
    if (renderedEmailId === e.id) return; // keep iframe state across unrelated re-renders
    renderedEmailId = e.id;
    const f = splitFrom(e.from);
    $('rSubject').textContent = e.subject || '(no subject)';
    $('rFrom').innerHTML = f.name && f.name !== f.addr ? `<span style="font-weight:500">${esc(f.name)}</span> <span class="muted">&lt;${esc(f.addr)}&gt;</span>` : esc(f.addr);
    $('rTo').textContent = e.to;
    $('rDate').textContent = fullDate(e.timestamp);
    if (isHtml(e.body)) {
        $('rBody').innerHTML = '';
        const frame = Object.assign(document.createElement('iframe'), { className: 'body-html', srcdoc: e.body, title: 'Email content' });
        // allow-same-origin (no scripts) so we can size the frame to its content.
        frame.setAttribute('sandbox', 'allow-same-origin allow-popups allow-popups-to-escape-sandbox');
        frame.onload = () => {
            try {
                frame.contentDocument.querySelectorAll('a').forEach((a) => { a.target = '_blank'; a.rel = 'noopener noreferrer'; });
                frame.style.height = frame.contentDocument.documentElement.scrollHeight + 20 + 'px';
            } catch { /* sizing is cosmetic */ }
        };
        $('rBody').appendChild(frame);
    } else {
        $('rBody').innerHTML = `<div class="body-text">${esc(e.body)}</div>`;
    }
    const atts = e.attachments || [];
    $('rAtts').hidden = !atts.length;
    $('rAttLabel').textContent = `Attachments · ${atts.length}`;
    $('rAttList').innerHTML = atts.map((a, i) => `
        <div class="att"><div><div class="name">${esc(a.filename)}</div><div class="info">${fileSize(a.size)} · ${esc(a.content_type)}</div></div>
        <button class="btn sm" data-att="${i}">Download</button></div>`).join('');
}

function renderHooks() {
    $('hookAddr').textContent = S.address;
    $('hookCount').textContent = String(S.webhooks.length);
    $('hookList').innerHTML = S.webhooks.map((h) => `
        <div class="hook">
            <div class="info">
                <div class="url">${esc(h.webhook_url)}</div>
                <div class="evs">${h.events.map((e) => `<span class="badge chip">${esc(e[0].toUpperCase() + e.slice(1))}</span>`).join('')}</div>
            </div>
            <span class="badge ${h.enabled ? 'ok' : 'off'}">${h.enabled ? 'Enabled' : 'Disabled'}</span>
            <div class="acts">
                <button class="btn sm" data-hook="${esc(h.id)}" data-act="test">Send test</button>
                <button class="btn sm" data-hook="${esc(h.id)}" data-act="edit">Edit</button>
                <button class="btn sm" data-hook="${esc(h.id)}" data-act="toggle">${h.enabled ? 'Disable' : 'Enable'}</button>
                <button class="btn sm danger" data-hook="${esc(h.id)}" data-act="delete">Delete</button>
            </div>
        </div>`).join('') || '<div class="empty">No webhooks for this mailbox yet.</div>';
}

// ---------------------------------------------------------------- events

function bind() {
    const go = (raw) => openMailbox(raw);
    $('openBtn').onclick = () => go($('openInput').value);
    $('openInput').onkeydown = (e) => { if (e.key === 'Enter') go($('openInput').value); };
    $('localBtn').onclick = () => go($('localInput').value || randomLocal());
    $('localInput').onkeydown = (e) => { if (e.key === 'Enter') go($('localInput').value || randomLocal()); };
    $('randomBtn').onclick = $('randomBtn2').onclick = () => go(randomLocal());

    $('copyBtn').onclick = async () => {
        try { await navigator.clipboard.writeText(S.address); } catch { /* clipboard blocked */ }
        $('copyBtn').textContent = 'Copied';
        setTimeout(() => { $('copyBtn').textContent = 'Copy'; }, 1500);
    };
    $('claimBtn').onclick = () => { if (!S.adminMode) openModal(S.claimed ? 'release' : 'claim'); };
    $('composeBtn').onclick = $('composeBtnM').onclick = () => openCompose(null);
    $('loginBtn').onclick = () => openModal('login');
    $('logoutBtn').onclick = logout;

    const onNav = (e) => {
        const b = e.target.closest('[data-view]');
        if (!b || b.disabled) return;
        if (b.dataset.view === 'admin') showAdmin(); else setView(b.dataset.view);
    };
    $('nav').onclick = $('tabs').onclick = onNav;

    $('menuBtn').onclick = () => { $('app').classList.add('drawer'); $('backdrop').hidden = false; };
    $('closeDrawer').onclick = $('backdrop').onclick = closeDrawer;

    $('recents').onclick = (e) => { const b = e.target.closest('[data-open]'); if (b) go(b.dataset.open); };
    $('rows').onclick = (e) => {
        if (e.target.id === 'adminMore') return loadMoreAdmin();
        const r = e.target.closest('.mrow');
        if (!r) return;
        if (r.dataset.admin) openAdminInbox(r.dataset.admin);
        else if (r.dataset.result) openSearchResult(r.dataset.result);
        else selectEmail(r.dataset.email);
    };
    $('backToAdmin').onclick = showAdmin;
    $('searchBox').onsubmit = (e) => { e.preventDefault(); runSearch(); };
    $('searchInput').oninput = (e) => { if (!e.target.value) runSearch(); };

    $('backBtn').onclick = () => { S.sel = null; render(); };
    $('replyBtn').onclick = () => openCompose(S.emails.find((e) => e.id === S.sel));
    $('deleteBtn').onclick = () => openModal('delete');
    $('rAttList').onclick = (e) => {
        const b = e.target.closest('[data-att]');
        const em = S.emails.find((x) => x.id === S.sel);
        if (b && em) download(em.attachments[+b.dataset.att]);
    };

    $('addHookBtn').onclick = () => openHookForm(null);
    $('hookCancel').onclick = closeHookForm;
    $('hookForm').onsubmit = saveHook;
    $('hookUrl').oninput = () => { $('hookErr').hidden = true; };
    $('hookList').onclick = (e) => { const b = e.target.closest('[data-hook]'); if (b) hookAction(b.dataset.hook, b.dataset.act); };
    $('payload').textContent = JSON.stringify({ event: 'arrival', mailbox: 'user@example.com', webhook_id: 'webhook-uuid', timestamp: '2024-01-01T00:00:00Z',
        email: { id: 'email-uuid', to: 'user@example.com', from: 'sender@example.com', subject: 'Email Subject', body: 'Email content', timestamp: '2024-01-01T00:00:00Z', attachments: 0 } }, null, 2);

    $('modalForm').onsubmit = (e) => { e.preventDefault(); submitModal(); };
    $('mCancel').onclick = () => {
        if (S.modal === 'unlock') $('openInput').value = localPart(S.address);
        closeModal();
    };
    $('modal').oncancel = (e) => { if ($('mCancel').hidden) e.preventDefault(); else S.modal = null; };
    $('toRegisterBtn').onclick = () => openModal('register');
    $('toLoginBtn').onclick = () => openModal('login');
    $('modalForm').oninput = () => { $('mErr').hidden = true; };
    $('toastClose').onclick = () => { $('toast').hidden = true; };

    document.addEventListener('visibilitychange', () => { if (!document.hidden) resync(); });
    window.addEventListener('focus', resync);
    window.addEventListener('popstate', loadFromUrl);
    setInterval(() => { if (S.address || S.view === 'admin') render(); }, 60000); // refresh relative times
}

function loadFromUrl() {
    const mb = new URLSearchParams(location.search).get('mailbox');
    if (mb && normalize(mb) !== S.address) openMailbox(mb);
    else if (!mb && S.address && !S.adminMode) { closeMailbox(); render(); }
}

(async () => {
    bind();
    await initAuth();
    render();
    if (needsLogin()) openModal(S.registrationOpen ? 'register' : 'login');
    else loadFromUrl();
    if ('Notification' in window && Notification.permission === 'default') Notification.requestPermission();
})();
