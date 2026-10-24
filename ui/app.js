// Launcher UI. All disk and network work happens in Rust (src-tauri/src/commands.rs); this file
// only decides what to show and which command a button calls.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const RING_LENGTH = 326.7; // 2 * PI * r for r = 52 in index.html
const $ = (id) => document.getElementById(id);

const state = {
  info: null,
  releases: [],
  releasesError: null,
  selectedTag: null,
  task: null, // { label, percent } while installing / uninstalling
};

// ---------- what the main button does ----------

function latest() {
  return state.releases[0] || null;
}

function primaryAction() {
  const installed = state.info?.installedVersion;
  const canPlay = state.info?.canPlay;
  const newest = latest();
  if (state.task) return { label: state.task.label, disabled: true };
  if (!state.info) return { label: 'Checking…', disabled: true };
  if (newest && (!installed || !canPlay)) return { label: 'Install', run: () => runInstall(newest.tag_name) };
  if (newest && installed !== newest.tag_name) return { label: 'Update', run: () => runInstall(newest.tag_name) };
  if (canPlay) return { label: 'Play', play: true, run: runPlay };
  if (state.releasesError) return { label: 'Offline', disabled: true };
  return { label: 'No release yet', disabled: true };
}

function statusText() {
  const installed = state.info?.installedVersion;
  const newest = latest();
  if (state.task) return state.task.label;
  if (state.releasesError) return installed ? 'Offline — playing installed version' : 'Cannot reach GitHub';
  if (!newest) return 'No release published yet';
  if (!installed) return 'Not installed';
  if (installed !== newest.tag_name) return `Update available: ${newest.tag_name}`;
  return 'Up to date';
}

// ---------- rendering ----------

function render() {
  renderSidebar();
  renderNews();
  renderHistory();
}

function renderSidebar() {
  const action = primaryAction();
  const button = $('primary');
  button.textContent = action.label;
  button.disabled = Boolean(action.disabled);
  button.classList.toggle('play', Boolean(action.play));
  button.onclick = action.run || null;

  $('status-line').textContent = statusText();
  $('v-installed').textContent = state.info?.installedVersion || '—';
  $('v-latest').textContent = latest()?.tag_name || '—';
  $('repair').disabled = !state.info?.installedVersion || Boolean(state.task);
  $('uninstall').disabled = !state.info?.installedVersion || Boolean(state.task);
  renderRing();
}

function renderRing() {
  const upToDate = state.info?.canPlay && state.info.installedVersion === latest()?.tag_name;
  let percent = 0;
  let label = '—';
  if (state.task) {
    percent = state.task.percent;
    label = `${Math.round(percent)}%`;
  } else if (upToDate || (state.info?.canPlay && state.releasesError)) {
    percent = 100;
    label = 'Ready';
  }
  $('ring').classList.toggle('ready', !state.task && percent === 100);
  $('ring-fill').style.strokeDashoffset = String(RING_LENGTH * (1 - percent / 100));
  $('ring-label').textContent = label;
}

function selectedRelease() {
  return state.releases.find((r) => r.tag_name === state.selectedTag) || latest();
}

function renderNews() {
  const release = selectedRelease();
  $('news-title').textContent = release ? release.name || release.tag_name : 'No release yet';
  $('news-badge').hidden = !release || release !== latest();
  $('news-date').textContent = release ? formatDate(release.published_at) : '';
  $('news-more').hidden = !release;
  renderNotes($('news-body'), release ? release.body : emptyNewsText());
}

function emptyNewsText() {
  if (state.releasesError) return `Could not load releases.\n${state.releasesError}`;
  return 'Releases appear here once a version tag (e.g. v0.1.0) is pushed to the game repository.';
}

function renderHistory() {
  const container = $('history');
  container.replaceChildren();
  if (!state.releases.length) {
    const empty = document.createElement('p');
    empty.className = 'empty';
    empty.textContent = 'No versions published yet.';
    container.append(empty);
    return;
  }
  state.releases.forEach((release) => container.append(historyCard(release)));
}

function historyCard(release) {
  const card = document.createElement('button');
  card.type = 'button';
  card.className = 'card';
  card.classList.toggle('selected', release === selectedRelease());
  card.append(
    textNode('span', 'tag', cardTag(release)),
    textNode('strong', '', release.name || release.tag_name),
    textNode('span', 'summary', firstLine(release.body) || 'No notes'),
    textNode('time', 'summary', formatDate(release.published_at)),
  );
  card.onclick = () => {
    state.selectedTag = release.tag_name;
    render();
  };
  return card;
}

function cardTag(release) {
  if (release.tag_name === state.info?.installedVersion) return 'Installed';
  if (release === latest()) return 'Latest';
  return release.tag_name;
}

// Release notes come from GitHub, so they are only ever inserted as text, never as HTML.
function renderNotes(container, markdown) {
  container.replaceChildren();
  let list = null;
  for (const raw of (markdown || '').split(/\r?\n/)) {
    const line = raw.trim();
    const bullet = line.match(/^[-*]\s+(.*)$/);
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (!line) {
      list = null;
    } else if (bullet) {
      list = list || container.appendChild(document.createElement('ul'));
      list.append(textNode('li', '', cleanMarkdown(bullet[1])));
    } else {
      list = null;
      container.append(textNode(heading ? 'h4' : 'p', '', cleanMarkdown(heading ? heading[1] : line)));
    }
  }
  if (!container.childNodes.length) container.append(textNode('p', '', 'No release notes.'));
}

function renderTrailer() {
  const box = $('trailer');
  const id = state.info?.trailerYoutubeId;
  box.replaceChildren();
  if (id) {
    const frame = document.createElement('iframe');
    frame.src = `https://www.youtube-nocookie.com/embed/${encodeURIComponent(id)}`;
    frame.title = 'Trailer';
    frame.allow = 'encrypted-media; picture-in-picture; fullscreen';
    box.append(frame);
    return;
  }
  const placeholder = document.createElement('div');
  placeholder.className = 'placeholder';
  const logo = document.createElement('img');
  logo.src = 'logo.png';
  logo.alt = '';
  placeholder.append(logo, textNode('p', '', 'Trailer coming soon'));
  box.append(placeholder);
}

function renderBanner(status) {
  $('banner').hidden = !status;
  if (!status) return;
  $('banner').className = `banner ${status.level === 'warning' ? '' : status.level}`.trim();
  $('banner-text').textContent = status.message;
}

// ---------- small helpers ----------

function textNode(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  node.textContent = text;
  return node;
}

function cleanMarkdown(text) {
  return text.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1').replace(/\*\*|__|`/g, '');
}

function firstLine(markdown) {
  const line = (markdown || '').split(/\r?\n/).map((l) => l.trim()).find((l) => l && !l.startsWith('#'));
  return line ? cleanMarkdown(line.replace(/^[-*]\s+/, '')) : '';
}

function formatDate(iso) {
  if (!iso) return '';
  return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

let toastTimer = null;
function toast(message, ok = false) {
  const box = $('toast');
  box.textContent = message;
  box.classList.toggle('ok', ok);
  box.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { box.hidden = true; }, ok ? 3000 : 7000);
}

// ---------- actions ----------

async function refreshInfo() {
  state.info = await invoke('get_info');
  $('game-name').textContent = state.info.gameName;
  $('install-dir').value = state.info.installDir;
  $('meta-line').textContent = `github.com/${state.info.gameRepo}`;
}

async function refreshReleases() {
  try {
    state.releases = await invoke('list_releases');
    state.releasesError = null;
  } catch (error) {
    // Fails open: without GitHub the player can still launch the installed version.
    state.releasesError = String(error);
  }
}

async function refreshStatus() {
  try {
    renderBanner(await invoke('get_status'));
  } catch {
    // Fails open: no banner is the right default when the status file is unreachable.
    renderBanner(null);
  }
}

async function checkForUpdates() {
  state.info = null;
  render();
  await Promise.all([refreshInfo(), refreshReleases(), refreshStatus()]);
  render();
}

async function runTask(label, command, args, successMessage) {
  state.task = { label, percent: 0 };
  render();
  try {
    await invoke(command, args);
    toast(successMessage, true);
  } catch (error) {
    toast(String(error));
  } finally {
    state.task = null;
    await refreshInfo();
    render();
  }
}

function runInstall(tag) {
  const verb = state.info?.installedVersion ? 'Updated' : 'Installed';
  return runTask('Starting…', 'install', { tag }, `${verb} to ${tag}`);
}

async function runPlay() {
  try {
    await invoke('play');
  } catch (error) {
    toast(String(error));
  }
}

function onProgress({ payload }) {
  if (!state.task) return;
  if (payload.stage === 'download') {
    state.task.percent = (payload.done / payload.total) * 90; // last 10% is unpacking
    state.task.label = `Downloading ${Math.round((payload.done / payload.total) * 100)}%`;
  } else if (payload.stage === 'extract') {
    state.task.percent = 95;
    state.task.label = 'Installing…';
  }
  render();
}

// Uninstall asks for a second click instead of a popup, so it works the same on every OS.
let uninstallArmed = null;
function onUninstallClick() {
  const button = $('uninstall');
  if (!uninstallArmed) {
    button.textContent = 'Click again to confirm';
    button.classList.add('confirm');
    uninstallArmed = setTimeout(disarmUninstall, 4000);
    return;
  }
  disarmUninstall();
  $('settings').close();
  runTask('Uninstalling…', 'uninstall', {}, 'Game uninstalled');
}

function disarmUninstall() {
  clearTimeout(uninstallArmed);
  uninstallArmed = null;
  $('uninstall').textContent = 'Uninstall';
  $('uninstall').classList.remove('confirm');
}

async function invokeOrToast(command, args, successMessage) {
  try {
    await invoke(command, args);
    if (successMessage) toast(successMessage, true);
    return true;
  } catch (error) {
    toast(String(error));
    return false;
  }
}

function releasesPage() {
  return `https://github.com/${state.info?.gameRepo}/releases`;
}

function wireButtons() {
  $('link-releases').onclick = () => invokeOrToast('open_repo_page', { url: `${releasesPage()}/` });
  $('link-folder').onclick = () => invokeOrToast('open_install_folder');
  $('open-folder').onclick = () => invokeOrToast('open_install_folder');
  $('news-more').onclick = () => invokeOrToast('open_repo_page', { url: selectedRelease().html_url });
  $('open-settings').onclick = () => $('settings').showModal();
  $('check-updates').onclick = () => { $('settings').close(); checkForUpdates(); };
  $('repair').onclick = () => {
    $('settings').close();
    runTask('Starting…', 'install', { tag: state.info.installedVersion }, 'Game repaired');
  };
  $('uninstall').onclick = onUninstallClick;
  $('settings').addEventListener('close', disarmUninstall);
  $('save-dir').onclick = async () => {
    if (await invokeOrToast('set_install_dir', { path: $('install-dir').value }, 'Install folder saved')) {
      await refreshInfo();
      render();
    }
  };
}

async function start() {
  wireButtons();
  await listen('install-progress', onProgress);
  await refreshInfo();
  renderTrailer();
  render();
  await checkForUpdates();
}

start().catch((error) => toast(`Launcher failed to start: ${error}`));
