import React, { Suspense, lazy } from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter, Route, Routes, useParams, Navigate, useLocation } from 'react-router-dom'
import Legal, { isLegalPath } from './pages/Legal'
// Polices servies par POP (pas par Google) : aucune adresse IP de visiteur n'est envoyée à un tiers.
import '@fontsource/plus-jakarta-sans/400.css'
import '@fontsource/plus-jakarta-sans/500.css'
import '@fontsource/plus-jakarta-sans/600.css'
import '@fontsource/plus-jakarta-sans/700.css'
import '@fontsource/geist-mono/400.css'
import '@fontsource/geist-mono/500.css'
import './styles.css'
import { AppProvider, useApp } from './state'
import Layout from './components/Layout'
import { Login, Signup, Onboarding, InviteAccept, PublicPage, ResetPassword, VerifyEmail, Activate, Welcome, FirstPassword } from './pages/Auth'
import Overview from './pages/Overview'
import Home from './pages/Home'
import { MapScope, StudioScope, Legacy } from './components/Scopes'
import Pages from './pages/Pages'
import Board from './pages/Board'
import Plan from './pages/Plan'
import Tasks from './pages/Tasks'
import Launch from './pages/Launch'
import Money from './pages/Money'
import Team, { Account } from './pages/Team'
import { PageHead } from './components/ui'
import Help from './pages/Help'
import Settings from './pages/Settings'
import { Market, Trending, Launches } from './pages/Analysis'
import Ideas from './pages/Ideas'
import MapsToMake from './pages/MapsToMake'
import { BillingSettings, Paywall } from './components/Billing'
import { AnalyseGate, AnalyseVideos, StudioMarket } from './components/Access'
import { tr } from './i18n/t'
const Landing = lazy(() => import('./pages/Landing'))
const Admin = lazy(() => import('./pages/Admin'))
const Authorize = lazy(() => import('./pages/Authorize'))
const Organizer = lazy(() => import('./pages/Organizer'))
const L = (C: any) => <Suspense fallback={<main className="content"><p className="muted" style={{ padding: 28 }}>{tr("Chargement…")}</p></main>}><C /></Suspense>

function Gate() {
  const { me, loading } = useApp()
  const loc = useLocation()
  // Pages légales : lisibles avec ou sans compte, sans passer par l'application.
  if (isLegalPath(loc.pathname)) return <Legal />
  if (loading) return <div className="auth" />
  if (!me) return <Routes>
    <Route path="/" element={<Suspense fallback={<div className="auth" />}><Landing /></Suspense>} />
    <Route path="/inscription" element={<Signup />} />
    <Route path="*" element={<Login />} />
  </Routes>
  // Connexion OAuth d'une IA (#285) : la page « Autoriser » passe avant tout le reste de l'application.
  if (loc.pathname === '/autoriser' && !me.must_change_password) return <Suspense fallback={<div className="auth" />}><Authorize /></Suspense>
  if (me.must_change_password) return <FirstPassword />
  if (me.billing_state === 'expired') return <Paywall />
  // Connecté depuis la page de connexion : on part sur l'accueil (l'adresse ne reste pas « /connexion »).
  if (['/connexion', '/inscription'].includes(loc.pathname)) return <Navigate to="/" replace />
  if (me.studios.length === 0) return <Onboarding />
  if (me.studios.every(s => s.maps.length === 0)) return <Welcome />
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Home />} />
        <Route path="map/:mid" element={<MapScope />}>
          <Route index element={<Overview />} />
          <Route path="pages" element={<Pages />} />
          <Route path="pages/:pid" element={<Pages />} />
          <Route path="tableau" element={<Board />} />
          <Route path="tableau/:bid" element={<Board />} />
          <Route path="planifier" element={<Plan />} />
          <Route path="taches" element={<Tasks />} />
          <Route path="organiseur" element={<Navigate to="/parametres?onglet=uefn" replace />} />
          <Route path="ia" element={<Navigate to="/parametres?onglet=ia" replace />} />
          <Route path="lancer" element={<Launch />} />
          <Route path="remunerer" element={<Money />} />
          <Route path="*" element={<Overview />} />
        </Route>
        <Route path="studio/:sid" element={<StudioScope />}>
          <Route index element={<Team tab="members" />} />
          <Route path="equipe" element={<Team tab="members" />} />
          <Route path="integrations" element={<Team tab="integ" />} />
          <Route path="maps" element={<Team tab="maps" />} />
          <Route path="parametres" element={<Team tab="settings" />} />
          <Route path="marche" element={<StudioMarket />} />
          <Route path="tendances" element={<Navigate to="../marche?v=videos" replace />} />
        </Route>
        <Route path="aide" element={<Help />} />
        <Route path="admin" element={L(Admin)} />
        <Route path="parametres" element={<Settings />} />
        <Route path="analyse/a-faire" element={<AnalyseGate title={tr("Maps à faire")}><MapsToMake /></AnalyseGate>} />
        <Route path="analyse/idees" element={<AnalyseGate title={tr("Idées de maps")}><Ideas /></AnalyseGate>} />
        <Route path="analyse/marche" element={<AnalyseGate title={tr("Analyse du marché")}><Market /></AnalyseGate>} />
        <Route path="analyse/tendances" element={<AnalyseGate title={tr("Maps qui marchent")}><Trending /></AnalyseGate>} />
        <Route path="analyse/lancements" element={<AnalyseGate title={tr("Meilleurs démarrages")}><Launches /></AnalyseGate>} />
        <Route path="analyse/videos" element={<AnalyseVideos />} />
        {['pages', 'pages/:pid', 'tableau', 'tableau/:bid', 'planifier', 'taches', 'organiseur', 'ia', 'lancer', 'remunerer', 'equipe', 'analyse/tiktok'].map(p => <Route key={p} path={p} element={<Legacy />} />)}
        <Route path="*" element={<Home />} />
      </Route>
    </Routes>
  )
}

function InviteRoute() {
  const { token } = useParams()
  return <InviteAccept token={token!} />
}
function ResetRoute() {
  const { token } = useParams()
  return <ResetPassword token={token!} />
}
function VerifyRoute() {
  const { token } = useParams()
  return <VerifyEmail token={token!} />
}
function LinkRoute() {
  const { token } = useParams()
  return <PublicPage token={token!} />
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <AppProvider>
        <Routes>
          <Route path="/invitation/:token" element={<InviteRoute />} />
          <Route path="/lien/:token" element={<LinkRoute />} />
          <Route path="/reinitialiser/:token" element={<ResetRoute />} />
          <Route path="/verifier-email/:token" element={<VerifyRoute />} />
          <Route path="/activer" element={<Activate />} />
          <Route path="/*" element={<Gate />} />
        </Routes>
      </AppProvider>
    </BrowserRouter>
  </React.StrictMode>,
)
