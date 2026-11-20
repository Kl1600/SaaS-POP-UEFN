// Page locale de POP Forge : demande l'adresse de POP, la fait vérifier par Forge, puis Forge ouvre POP.
const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

function montrer(adresse, erreur) {
  $('intro').textContent = erreur
    ? 'POP ne s’est pas ouvert. Corrigez l’adresse ou réessayez.'
    : 'Indiquez l’adresse de votre POP. Vous vous connecterez ensuite avec votre compte habituel.';
  $('adresse').value = adresse || '';
  $('erreur').textContent = erreur || '';
  $('f').hidden = false;
  $('adresse').focus();
}

$('f').addEventListener('submit', async (e) => {
  e.preventDefault();
  $('ok').disabled = true;
  $('ok').textContent = 'Vérification…';
  $('erreur').textContent = '';
  try {
    await invoke('connect', { address: $('adresse').value });
  } catch (err) {
    $('erreur').textContent = String(err);
    $('ok').disabled = false;
    $('ok').textContent = 'Se connecter';
  }
});

(async () => {
  const changer = new URLSearchParams(location.search).has('changer');
  try {
    const r = await invoke('start', { change: changer });
    // Adresse enregistrée qui répond : Forge ouvre POP, cette page disparaît.
    if (r.address && !r.error && !changer) return;
    montrer(r.address, r.error);
  } catch (err) {
    montrer('', String(err));
  }
})();
