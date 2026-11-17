import React, { Suspense, lazy } from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter, Route, Routes, useParams } from 'react-router-dom'
import './styles.css'
import { AppProvider, useApp } from './state'
import Layout from './components/Layout'
import { Login, Signup, Onboarding, InviteAccept, PublicPage, ResetPassword, Welcome, FirstPassword } from './pages/Auth'
import Overview from './pages/Overview'
import Pages from './pages/Pages'
import Board from './pages/Board'
import Plan from './pages/Plan'
import Tasks from './pages/Tasks'
import Launch from './pages/Launch'
import Money from './pages/Money'
import Team, { Account } from './pages/Team'
import { PageHead } from './components/ui'
import Help from './pages/Help'
import AI from './pages/AI'
import { Market, TikTok, Trending, Launches } from './pages/Analysis'
import { tr } from './i18n/t'
const Landing = lazy(() => import('./pages/Landing'))
const Admin = lazy(() => import('./pages/Admin'))
const Organizer = lazy(() => import('./pages/Organizer'))
const L = (C: any) => <Suspense fallback={<main className="content"><p className="muted" style={{ padding: 28 }}>{tr("Chargement…")}</p></main>}><C /></Suspense>

function Gate() {
  const { me, loading } = useApp()
  if (loading) return <div className="auth" />
  if (!me) return <Routes>
    <Route path="/" element={<Suspense fallback={<div className="auth" />}><Landing /></Suspense>} />
    <Route path="/inscription" element={<Signup />} />
    <Route path="*" element={<Login />} />
  </Routes>
  if (me.must_change_password) return <FirstPassword />
  if (me.studios.length === 0) return <Onboarding />
  if (me.studios.every(s => s.maps.length === 0)) return <Welcome />
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Overview />} />
        <Route path="pages" element={<Pages />} />
        <Route path="pages/:pid" element={<Pages />} />
        <Route path="tableau" element={<Board />} />
        <Route path="tableau/:bid" element={<Board />} />
        <Route path="planifier" element={<Plan />} />
        <Route path="taches" element={<Tasks />} />
        <Route path="lancer" element={<Launch />} />
        <Route path="remunerer" element={<Money />} />
        <Route path="equipe" element={<Team />} />
        <Route path="organiseur" element={L(Organizer)} />
        <Route path="aide" element={<Help />} />
        <Route path="admin" element={L(Admin)} />
        <Route path="parametres" element={<main className="content"><PageHead title={tr("Paramètres")} help={tr("Votre compte : nom, mot de passe, données personnelles.")} /><Account /></main>} />
        <Route path="ia" element={<AI />} />
        <Route path="analyse/marche" element={<Market />} />
        <Route path="analyse/tiktok" element={<TikTok />} />
        <Route path="analyse/tendances" element={<Trending />} />
        <Route path="analyse/lancements" element={<Launches />} />
        <Route path="*" element={<Overview />} />
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
          <Route path="/*" element={<Gate />} />
        </Routes>
      </AppProvider>
    </BrowserRouter>
  </React.StrictMode>,
)
