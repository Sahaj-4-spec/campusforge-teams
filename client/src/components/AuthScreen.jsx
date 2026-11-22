import { useState } from "react";
import { ArrowRight, Eye, EyeOff, LockKeyhole, Mail, Sparkles, UserRound, Zap } from "lucide-react";
import { authApi, storeToken } from "../lib/api.js";

export default function AuthScreen({ onAuthenticated }) {
  const [mode, setMode] = useState("login");
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const isRegister = mode === "register";

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

  return (
    <main className="auth-screen">
      <section className="auth-card">
        <a className="brand auth-brand" href="/" aria-label="CampusForge Teams">
          <span className="brand-mark"><Zap size={21} fill="currentColor" /></span>
          <span className="brand-name">CampusForge<small>TEAMS WORKSPACE</small></span>
        </a>
        <div className="auth-intro">
          <span className="auth-sparkle"><Sparkles size={17} /></span>
          <p className="eyebrow">{isRegister ? "CREATE YOUR WORKSPACE" : "WELCOME BACK"}</p>
          <h1>{isRegister ? "Build something together." : "Good to see you again."}</h1>
          <p>{isRegister
            ? "Create your account and start organizing your team's work."
            : "Sign in to continue to your projects, tasks, and team."}</p>
        </div>

        <form className="auth-form" onSubmit={submit}>
          {isRegister && (
            <label className="field-group">
              <span>Full name</span>
              <span className="auth-input-wrap"><UserRound size={16} /><input value={fullName} onChange={(event) => setFullName(event.target.value)} placeholder="Your name" autoComplete="name" minLength={2} maxLength={80} required /></span>
            </label>
          )}
          <label className="field-group">
            <span>Email address</span>
            <span className="auth-input-wrap"><Mail size={16} /><input type="email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@example.com" autoComplete="email" required /></span>
          </label>
          <label className="field-group">
            <span>Password</span>
            <span className="auth-input-wrap"><LockKeyhole size={16} /><input type={showPassword ? "text" : "password"} value={password} onChange={(event) => setPassword(event.target.value)} placeholder={isRegister ? "At least 8 characters" : "Your password"} autoComplete={isRegister ? "new-password" : "current-password"} minLength={isRegister ? 8 : 1} maxLength={128} required /><button className="auth-password-toggle" type="button" onClick={() => setShowPassword((value) => !value)} aria-label={showPassword ? "Hide password" : "Show password"}>{showPassword ? <EyeOff size={16} /> : <Eye size={16} />}</button></span>
          </label>
          {error && <p className="auth-error" role="alert">{error}</p>}
          <button className="primary-button auth-submit" type="submit" disabled={busy}>
            {busy ? "Please wait..." : isRegister ? "Create account" : "Sign in"} <ArrowRight size={16} />
          </button>
        </form>

        <p className="auth-switch">
          {isRegister ? "Already have an account?" : "New to CampusForge?"}
          <button type="button" onClick={() => { setMode(isRegister ? "login" : "register"); setError(""); }}>
            {isRegister ? "Sign in" : "Create an account"}
          </button>
        </p>
        <p className="auth-security">Your password is securely hashed by the CampusForge API. Sessions expire after 24 hours.</p>
      </section>
      <div className="auth-side-note"><span /> Plan better. Collaborate smarter. Build together.</div>
    </main>
  );
}
