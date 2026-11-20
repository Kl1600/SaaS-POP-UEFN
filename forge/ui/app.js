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

// ---- Relier un projet UEFN (UEFN Link dans Forge) ----
async function relierEcran() {
  $('intro').textContent = 'Reliez un projet UEFN à votre map POP.';
  $('r').hidden = false;
  $('etat').textContent = 'Relais : ' + await invoke('etat_relais');
  const liste = await invoke('projets');
  const sel = $('projet');
  for (const p of liste) sel.add(new Option(p.split(/[\\/]/).pop() + ' — ' + p, p));
  sel.add(new Option('Autre dossier…', ''));
  const autre = () => { $('autre').hidden = sel.value !== ''; };
  sel.addEventListener('change', autre);
  autre();
  $('code').focus();
  setInterval(async () => { $('etat').textContent = 'Relais : ' + await invoke('etat_relais'); }, 2000);
}

$('r').addEventListener('submit', async (e) => {
  e.preventDefault();
  $('okr').disabled = true;
  $('okr').textContent = 'Liaison…';
  $('erreurr').textContent = '';
  const projet = $('projet').value || $('autre').value;
  try {
    const etat = await invoke('relier', { code: $('code').value, projet });
    $('etat').textContent = 'Relais : ' + etat;
    $('okr').textContent = 'Relié';
  } catch (err) {
    $('erreurr').textContent = String(err);
    $('okr').disabled = false;
    $('okr').textContent = 'Relier ce projet';
  }
});

$('retour').addEventListener('click', async () => {
  const r = await invoke('start', { change: false });
  if (r.error) $('erreurr').textContent = r.error;
});

(async () => {
  if (new URLSearchParams(location.search).has('relier')) return relierEcran();
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
