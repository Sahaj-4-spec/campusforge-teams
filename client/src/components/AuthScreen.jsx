import { useEffect, useState } from "react";
import {
  ArrowRight,
  Eye,
  EyeOff,
  Github,
  LockKeyhole,
  Mail,
  ShieldCheck,
  Sparkles,
  UserRound
} from "lucide-react";
import { authApi, storeToken } from "../lib/api.js";

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

export default function AuthScreen({ onAuthenticated }) {
  const [mode, setMode] = useState("login");
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [busyProvider, setBusyProvider] = useState("");
  const [error, setError] = useState("");

  const isRegister = mode === "register";

  useEffect(() => {
    const fragment = new URLSearchParams(window.location.hash.replace(/^#/, ""));
    const oauthToken = fragment.get("oauth_token");

    if (oauthToken) {
      window.history.replaceState({}, "", window.location.pathname + window.location.search);
      storeToken(oauthToken);
      onAuthenticated(oauthToken);
      return;
    }

    const query = new URLSearchParams(window.location.search);
    const oauthError = query.get("oauth_error");
    if (oauthError) {
      query.delete("oauth_error");
      const nextSearch = query.toString();
      window.history.replaceState(
        {},
        "",
        window.location.pathname + (nextSearch ? "?" + nextSearch : "") + window.location.hash
      );
      setError(oauthError);
    }
  }, [onAuthenticated]);

  async function submit(event) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      const result = isRegister
        ? await authApi.register({ full_name: fullName.trim(), email: email.trim(), password })
        : await authApi.login({ email: email.trim(), password });
      storeToken(result.token);
      onAuthenticated(result.token);
    } catch (requestError) {
      setError(requestError.message || "Unable to authenticate. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  function startOAuth(provider) {
    setError("");
    setBusyProvider(provider);
    window.location.assign(authApi.oauthStartUrl(provider));
  }

  function switchMode() {
    setMode(isRegister ? "login" : "register");
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
          <p className="eyebrow">{isRegister ? "START COLLABORATING" : "WELCOME BACK"}</p>
          <h1>{isRegister ? "Create your account" : "Welcome back"}</h1>
          <p>{isRegister
            ? "Set up your account and bring your team's work into one place."
            : "Sign in to continue to your projects, tasks, and team."}</p>
        </div>

        <form className="auth-form" onSubmit={submit}>
          {isRegister && (
            <label className="field-group">
              <span>Full name</span>
              <span className="auth-input-wrap">
                <UserRound size={17} aria-hidden="true" />
                <input
                  value={fullName}
                  onChange={(event) => setFullName(event.target.value)}
                  placeholder="Your name"
                  autoComplete="name"
                  minLength={2}
                  maxLength={80}
                  required
                />
              </span>
            </label>
          )}
          <label className="field-group">
            <span>Email address</span>
            <span className="auth-input-wrap">
              <Mail size={17} aria-hidden="true" />
              <input
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder="you@example.com"
                autoComplete="email"
                required
              />
            </span>
          </label>
          <div className="field-group">
            <label htmlFor="auth-password">Password</label>
            <span className="auth-input-wrap">
              <LockKeyhole size={17} aria-hidden="true" />
              <input
                id="auth-password"
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder={isRegister ? "At least 8 characters" : "Enter your password"}
                autoComplete={isRegister ? "new-password" : "current-password"}
                minLength={isRegister ? 8 : 1}
                maxLength={128}
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
          </div>

          {error && <p className="auth-error" role="alert">{error}</p>}

          <button className="primary-button auth-submit" type="submit" disabled={busy || Boolean(busyProvider)}>
            {busy ? "Please wait..." : isRegister ? "Create account" : "Sign in"}
            <ArrowRight size={17} />
          </button>
        </form>

        <div className="auth-divider"><span>or continue with</span></div>

        <div className="social-auth-grid" aria-label="Social sign-in providers">
          {providers.map(({ id, label, icon: Icon }) => (
            <button
              className={"social-auth-button social-" + id}
              type="button"
              key={id}
              onClick={() => startOAuth(id)}
              disabled={Boolean(busyProvider) || busy}
              aria-label={"Continue with " + label}
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

        <p className="auth-security"><ShieldCheck size={14} /> Protected sign-in. Your CampusForge session expires after 24 hours.</p>
      </section>

      <aside className="auth-side-panel">
        <div className="auth-side-grid" aria-hidden="true" />
        <div className="auth-side-orbit auth-orbit-one" aria-hidden="true" />
        <div className="auth-side-orbit auth-orbit-two" aria-hidden="true" />
        <div className="auth-panel-top">
          <img src="/campusforge-mark.svg" alt="" />
          <span>BUILT FOR SHARED MOMENTUM</span>
        </div>
        <div className="auth-panel-copy">
          <div className="auth-panel-pill"><Sparkles size={13} /> YOUR TEAM'S NEXT CHAPTER</div>
          <h2>{isRegister ? "Make great work happen together." : "Hello, there."}</h2>
          <p>Bring ideas, projects, and people together. Keep every task visible and every team moving forward.</p>
          <div className="auth-panel-features">
            <span><i>01</i> Clear project spaces</span>
            <span><i>02</i> Shared team momentum</span>
            <span><i>03</i> Progress everyone can see</span>
          </div>
          <button className="auth-panel-cta" type="button" onClick={switchMode}>
            {isRegister ? "Back to sign in" : "Create your account"}
            <ArrowRight size={17} />
          </button>
        </div>
        <div className="auth-panel-footer">
          <span><span className="auth-live-dot" /> YOUR WORKSPACE STARTS HERE</span>
          <span>PLAN · BUILD · GROW</span>
        </div>
      </aside>
    </main>
  );
}
