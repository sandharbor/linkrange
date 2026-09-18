document.documentElement.dataset.sharedScript = 'loaded';

const status = document.querySelector('#script-status');
if (status) {
  status.textContent = 'Shared JavaScript loaded.';
}
