import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  ArrowRight,
  CalendarDays,
  Check,
  Eye,
  EyeOff,
  Github,
  LockKeyhole,
  Mail,
  MessageCircle,
  ShieldCheck,
  Sparkles,
  Users,
} from "lucide-react";
import { authApi } from "../lib/api.js";

function GoogleIcon() {
  return (
    <svg className="provider-google-glyph" viewBox="0 0 48 48" aria-hidden="true">
      <path fill="#4285F4" d="M43.6 24.5c0-1.4-.1-2.8-.4-4.1H24v7.8h11a9.4 9.4 0 0 1-4.1 6.1v5h6.6c3.9-3.6 6.1-8.7 6.1-14.8Z" />
      <path fill="#34A853" d="M24 44c5.5 0 10.1-1.8 13.5-4.8l-6.6-5c-1.8 1.2-4.1 1.9-6.9 1.9-5.3 0-9.8-3.6-11.4-8.4H5.8v5.2A20.4 20.4 0 0 0 24 44Z" />
      <path fill="#FBBC05" d="M12.6 27.7a12.3 12.3 0 0 1 0-7.4v-5.2H5.8a20.1 20.1 0 0 0 0 17.8l6.8-5.2Z" />
      <path fill="#EA4335" d="M24 11.9c3 0 5.7 1 7.8 3.1l5.8-5.8C34.1 5.9 29.5 4 24 4A20.4 20.4 0 0 0 5.8 15.1l6.8 5.2c1.6-4.8 6.1-8.4 11.4-8.4Z" />
    </svg>
  );
}

function DiscordIcon() {
  return (
    <svg className="provider-discord-glyph" viewBox="0 0 24 24" aria-hidden="true">
      <path fill="currentColor" d="M19.7 5.1a18.2 18.2 0 0 0-4.5-1.4l-.6 1.2a16.7 16.7 0 0 0-5.1 0l-.6-1.2a18.1 18.1 0 0 0-4.5 1.4C1.6 9.3.8 13.4 1.2 17.5a18.4 18.4 0 0 0 5.6 2.8l1.2-2a11.8 11.8 0 0 1-1.9-.9l.5-.4c3.6 1.7 7.5 1.7 11 0l.6.4c-.6.4-1.3.7-2 .9l1.2 2a18.4 18.4 0 0 0 5.6-2.8c.5-4.8-.8-8.8-3.3-12.4ZM8.8 14.9c-1.1 0-2-1-2-2.2s.9-2.2 2-2.2 2 1 2 2.2-.9 2.2-2 2.2Zm6.4 0c-1.1 0-2-1-2-2.2s.9-2.2 2-2.2 2 1 2 2.2-.9 2.2-2 2.2Z" />
    </svg>
  );
}

const providers = [
  { id: "google", label: "Google", icon: GoogleIcon },
  { id: "github", label: "GitHub", icon: Github },
  { id: "discord", label: "Discord", icon: DiscordIcon }
];

function getEmailError(value) {
  const email = value.trim();
  if (!email) return "Enter your email address.";
  if (!/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(email)) return "Enter a valid email address.";
  return "";
}

export default function AuthScreen({ onAuthenticated }) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [busyProvider, setBusyProvider] = useState("");
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState({});
  const [showRecoveryNotice, setShowRecoveryNotice] = useState(false);

  const location = useLocation();
  const navigate = useNavigate();
  const isRegister = location.pathname === "/signup";

  useEffect(() => {
    const query = new URLSearchParams(location.search);
    const hasOAuthError = query.has("oauth_error");
    const oauthError = query.get("oauth_error");
    if (hasOAuthError) {
      query.delete("oauth_error");
      setError(oauthError || "Authentication failed. Please try again.");
    }

    const nextPath = isRegister ? "/signup" : "/login";
    const nextSearch = query.toString();
    const nextHash = location.hash.includes("oauth_token=") ? "" : location.hash;
    const nextUrl = nextPath + (nextSearch ? "?" + nextSearch : "") + nextHash;
    const currentUrl = location.pathname + location.search + location.hash;

    if (nextUrl !== currentUrl) {
      navigate(nextUrl, { replace: true });
    }
  }, [isRegister, location.hash, location.pathname, location.search, navigate]);

  async function submit(event) {
    event.preventDefault();
    setError("");
    setShowRecoveryNotice(false);

    const nextFieldErrors = {
      email: getEmailError(email),
      password: password ? "" : "Enter your password."
    };
    setFieldErrors(nextFieldErrors);
    if (Object.values(nextFieldErrors).some(Boolean)) return;

    setBusy(true);
    try {
      await authApi.login({ email: email.trim(), password });
      onAuthenticated();
    } catch (requestError) {
      setError(requestError.message || "Unable to sign in. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  function handleEmailBlur() {
    setFieldErrors((current) => ({ ...current, email: getEmailError(email) }));
  }

  function handlePasswordBlur() {
    setFieldErrors((current) => ({ ...current, password: password ? "" : "Enter your password." }));
  }

  function updateEmail(value) {
    setEmail(value);
    setFieldErrors((current) => ({ ...current, email: "" }));
    setError("");
  }

  function updatePassword(value) {
    setPassword(value);
    setFieldErrors((current) => ({ ...current, password: "" }));
    setError("");
  }

  function startOAuth(provider) {
    setError("");
    setBusyProvider(provider);
    window.location.assign(authApi.oauthStartUrl(provider));
  }

  function switchMode() {
    navigate(isRegister ? "/login" : "/signup");
    setError("");
    setPassword("");
  }

  return (
    <main className="auth-screen auth-screen-split">
      <section className="auth-card">
        <a className="brand auth-brand" href="/" aria-label="CampusForge Teams home">
          <span className="brand-mark"><img src="/campusforge-mark.svg" alt="" /></span>
          <span className="brand-name">CampusForge<small>TEAMS WORKSPACE</small></span>
        </a>

        <div className="auth-intro">
          <p className="eyebrow">{isRegister ? "START COLLABORATING" : "WORKSPACE ACCESS"}</p>
          <h1>{isRegister ? "Create your account" : "Welcome back"}</h1>
          <p>{isRegister
            ? "Create your account with Google, GitHub, or Discord. We verify your email with the provider before creating your account."
            : "Sign in to continue to your projects, tasks, and team."}</p>
        </div>

        {!isRegister && (
          <form className="auth-form" onSubmit={submit} noValidate>
            <div className="field-group">
              <label htmlFor="auth-email">Email address</label>
              <span className="auth-input-wrap focus-within:ring-2 focus-within:ring-violet-200">
                <Mail size={17} aria-hidden="true" />
                <input
                  id="auth-email"
                  className="outline-none"
                  type="email"
                  value={email}
                  onChange={(event) => updateEmail(event.target.value)}
                  onBlur={handleEmailBlur}
                  placeholder="you@example.com"
                  autoComplete="email"
                  aria-invalid={Boolean(fieldErrors.email)}
                  aria-describedby={fieldErrors.email ? "auth-email-error" : undefined}
                  required
                />
              </span>
              {fieldErrors.email && <p className="auth-field-error" id="auth-email-error" role="alert">{fieldErrors.email}</p>}
            </div>
            <div className="field-group">
              <div className="auth-field-heading">
                <label htmlFor="auth-password">Password</label>
                <button
                  className="auth-forgot-link"
                  type="button"
                  onClick={() => { setShowRecoveryNotice(true); setError(""); }}
                  aria-expanded={showRecoveryNotice}
                  aria-controls="auth-recovery-notice"
                >
                  Forgot password?
                </button>
              </div>
              <span className="auth-input-wrap">
                <LockKeyhole size={17} aria-hidden="true" />
                <input
                  id="auth-password"
                  className="outline-none"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(event) => updatePassword(event.target.value)}
                  onBlur={handlePasswordBlur}
                  placeholder="Enter your password"
                  autoComplete="current-password"
                  minLength={1}
                  maxLength={128}
                  aria-invalid={Boolean(fieldErrors.password)}
                  aria-describedby={fieldErrors.password ? "auth-password-error" : undefined}
                  required
                />
                <button
                  className="auth-password-toggle"
                  type="button"
                  onClick={() => setShowPassword((value) => !value)}
                  aria-label={showPassword ? "Hide password" : "Show password"}
                >
                  {showPassword ? <EyeOff size={17} /> : <Eye size={17} />}
                </button>
              </span>
              {fieldErrors.password && <p className="auth-field-error" id="auth-password-error" role="alert">{fieldErrors.password}</p>}
              {showRecoveryNotice && (
                <p className="auth-recovery-notice" id="auth-recovery-notice" role="status">
                  Password reset is not set up yet. Try signing in with your connected Google, GitHub, or Discord account, or contact your workspace administrator.
                </p>
              )}
            </div>
            <button className="primary-button auth-submit" type="submit" disabled={busy || Boolean(busyProvider)}>
              {busy ? "Please wait..." : "Sign in"}
              <ArrowRight size={17} />
            </button>
          </form>
        )}
        {error && <p className="auth-error" role="alert">{error}</p>}

        <div className="auth-divider"><span>{isRegister ? "verify and sign up with" : "or continue with"}</span></div>

        <div className="social-auth-grid" aria-label="Social sign-in providers">
          {providers.map(({ id, label, icon: Icon }) => (
            <button
              className={"social-auth-button social-" + id}
              type="button"
              key={id}
              onClick={() => startOAuth(id)}
              disabled={Boolean(busyProvider) || busy}
              aria-label={(isRegister ? "Sign up with " : "Continue with ") + label}
            >
              {busyProvider === id ? <span className="social-spinner" /> : <Icon size={18} aria-hidden="true" />}
              <span>{busyProvider === id ? "Connecting..." : label}</span>
            </button>
          ))}
        </div>

        <p className="auth-switch">
          {isRegister ? "Already have an account?" : "New to CampusForge?"}
          <button type="button" onClick={switchMode}>
            {isRegister ? "Sign in" : "Create an account"}
          </button>
        </p>

        {isRegister ? (
          <p className="auth-security"><ShieldCheck size={14} /> New accounts require an email verified by Google, GitHub, or Discord.</p>
        ) : (
          <p className="auth-security"><ShieldCheck size={14} /> Protected sign-in. Your CampusForge session expires after 24 hours.</p>
        )}
      </section>

      <aside className="auth-preview-panel" aria-label="CampusForge product preview">
        <div className="auth-preview-topline">
          <span className="auth-preview-brand-mark"><img src="/campusforge-mark.svg" alt="" /></span>
          <span className="auth-preview-brand-name">CAMPUSFORGE TEAMS</span>
          <span className="auth-preview-pill"><Sparkles size={12} /> TEAM WORKSPACE</span>
        </div>

        <div className="auth-preview-copy">
          <p className="auth-preview-eyebrow">PLAN · TRACK · SHIP</p>
          <h2>Bring every project into focus.</h2>
          <p>Turn scattered tasks into shared momentum with boards, deadlines, and team conversations in one workspace.</p>
        </div>

        <section className="auth-board-preview" aria-label="Illustrative sample Kanban board">
          <header className="auth-board-header">
            <div>
              <span className="auth-board-kicker">SAMPLE PROJECT BOARD</span>
              <strong>Campus launch</strong>
            </div>
            <div className="auth-board-avatars" aria-label="Sample teammates">
              <span>SK</span><span>AM</span><span>JR</span>
            </div>
          </header>

          <div className="auth-board-columns">
            <section className="auth-board-column">
              <header><span className="auth-board-status status-todo" />To do <b>02</b></header>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-design">DESIGN</span>
                <strong>Map the onboarding flow</strong>
                <footer><span>◷ This week</span><span className="auth-task-avatar">SK</span></footer>
              </article>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-research">RESEARCH</span>
                <strong>Collect beta feedback</strong>
                <footer><span>◷ Nov 27</span><span className="auth-task-avatar task-avatar-blue">AM</span></footer>
              </article>
            </section>

            <section className="auth-board-column">
              <header><span className="auth-board-status status-progress" />In progress <b>02</b></header>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-build">BUILD</span>
                <strong>Build the team dashboard</strong>
                <footer><span>◷ Today</span><span className="auth-task-avatar task-avatar-orange">JR</span></footer>
              </article>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-product">PRODUCT</span>
                <strong>Connect the team calendar</strong>
                <footer><span>◷ Nov 28</span><span className="auth-task-avatar">SK</span></footer>
              </article>
            </section>

            <section className="auth-board-column">
              <header><span className="auth-board-status status-done" />Done <b>02</b></header>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-launch">LAUNCH</span>
                <strong>Set up the workspace</strong>
                <footer><span><Check size={11} /> Complete</span><span className="auth-task-avatar task-avatar-green">AM</span></footer>
              </article>
              <article className="auth-board-task">
                <span className="auth-task-tag tag-design">DESIGN</span>
                <strong>Approve brand direction</strong>
                <footer><span><Check size={11} /> Complete</span><span className="auth-task-avatar task-avatar-orange">JR</span></footer>
              </article>
            </section>
          </div>

          <footer className="auth-board-footer">
            <span><span className="auth-live-dot" /> Shared team view</span>
            <span>Sample data · Not a live board</span>
          </footer>
        </section>

        <div className="auth-preview-highlights">
          <span><CalendarDays size={15} /> Deadlines & milestones</span>
          <span><MessageCircle size={15} /> Team discussions</span>
          <span><Users size={15} /> Shared progress</span>
        </div>
      </aside>
    </main>
  );
}
