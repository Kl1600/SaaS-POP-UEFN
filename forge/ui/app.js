// Page locale de POP Forge. Forge ouvre lui-même pop-uefn.com (ou l'adresse enregistrée) ; cette page ne se voit que
// si POP est injoignable (#412), pour changer l'adresse (menu du profil) ou pour relier un projet UEFN.
const invoke = window.__TAURI__.core.invoke;
// Barre de titre (#78) : glisser déplace la fenêtre, double-clic l'agrandit, boutons à droite.
(() => {
  const barre = document.getElementById('barre');
  const fen = action => invoke('fenetre', { action }).catch(() => {});
  barre.addEventListener('mousedown', e => { if (e.button === 0 && e.detail === 1 && !e.target.closest('button')) fen('deplacer'); });
  barre.addEventListener('dblclick', e => { if (!e.target.closest('button')) fen('agrandir'); });
  barre.querySelectorAll('button').forEach(b => b.addEventListener('click', () => fen(b.dataset.action)));
})();
const $ = (id) => document.getElementById(id);

function montrer(adresse, erreur) {
  $('injoignable').hidden = true;
  $('intro').textContent = erreur
    ? 'POP ne s’est pas ouvert. Corrigez l’adresse ou réessayez.'
    : 'Adresse du serveur POP (pop-uefn.com, ou un POP installé ailleurs). Vous vous connecterez ensuite avec votre compte habituel.';
  $('adresse').value = adresse || '';
  $('erreur').textContent = erreur || '';
  $('f').hidden = false;
  $('adresse').focus();
}

// POP ne répond pas : pas de champ d'adresse, seulement « Réessayer » (et un nouvel essai toutes les 10 s).
let essai = null;
function injoignable(adresse, erreur) {
  $('f').hidden = true;
  $('intro').textContent = 'POP est injoignable. Vérifiez votre connexion à Internet.';
  $('detail').textContent = adresse + ' : ' + erreur;
  $('injoignable').hidden = false;
  clearInterval(essai);
  essai = setInterval(() => lancer(false, true), 10000);
}

async function lancer(changer, silencieux) {
  if (!silencieux) { $('reessayer').disabled = true; $('reessayer').textContent = 'Connexion…'; }
  try {
    const r = await invoke('start', { change: changer });
    if (changer) return montrer(r.address, null);
    // POP répond : Forge l'a ouvert, cette page disparaît.
    if (!r.error) { clearInterval(essai); return; }
    injoignable(r.address, r.error);
  } catch (err) {
    injoignable('POP', String(err));
  } finally {
    $('reessayer').disabled = false; $('reessayer').textContent = 'Réessayer';
  }
}

$('reessayer').addEventListener('click', () => lancer(false, false));
$('autre-serveur').addEventListener('click', () => { clearInterval(essai); lancer(true, false); });
$('officiel').addEventListener('click', () => { $('adresse').value = 'pop-uefn.com'; $('f').requestSubmit(); });

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
  lancer(new URLSearchParams(location.search).has('changer'), true);
})();
