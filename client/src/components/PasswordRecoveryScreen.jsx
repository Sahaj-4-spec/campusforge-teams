import { useState } from "react";
import { ArrowLeft, ArrowRight, LockKeyhole, Mail, ShieldCheck } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { authApi } from "../lib/api.js";

function getResetToken(search) {
  return new URLSearchParams(search).get("token") || "";
}

export default function PasswordRecoveryScreen() {
  const location = useLocation();
  const navigate = useNavigate();
  const isReset = location.pathname === "/reset-password";
  const token = getResetToken(location.search);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");

  async function submit(event) {
    event.preventDefault();
    setError("");
    setStatus("");

    if (isReset) {
      if (!token) {
        setError("This reset link is missing its token. Request a new link.");
        return;
      }
      if (password.length < 12) {
        setError("Use a password with at least 12 characters.");
        return;
      }
      if (password !== confirmPassword) {
        setError("The passwords do not match.");
        return;
      }
    } else {
      const trimmedEmail = email.trim();
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmedEmail)) {
        setError("Enter a valid email address.");
        return;
      }
    }

    setBusy(true);
    try {
      if (isReset) {
        const result = await authApi.confirmPasswordReset(token, password);
        setPassword("");
        setConfirmPassword("");
        setStatus(result?.message || "Your password has been reset. Sign in again.");
      } else {
        const result = await authApi.requestPasswordReset(email.trim());
        setStatus(result?.message || "If an account exists for that address, reset instructions have been sent.");
      }
    } catch (requestError) {
      setError(requestError.message || "Password recovery is temporarily unavailable. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="password-recovery-screen">
      <section className="password-recovery-card" aria-labelledby="password-recovery-title">
        <a className="brand password-recovery-brand" href="/" aria-label="CampusForge Teams home">
          <span className="brand-mark"><img src="/campusforge-mark.svg" alt="" /></span>
          <span className="brand-name">CampusForge<small>TEAMS WORKSPACE</small></span>
        </a>

        <div className="password-recovery-heading">
          <span className="password-recovery-icon">
            {isReset ? <LockKeyhole size={21} /> : <Mail size={21} />}
          </span>
          <p className="eyebrow">ACCOUNT RECOVERY</p>
          <h1 id="password-recovery-title">{isReset ? "Choose a new password" : "Reset your password"}</h1>
          <p>
            {isReset
              ? "Choose a strong password. Reset links expire after 30 minutes and can only be used once."
              : "Enter the email address linked to your account. If it exists, we’ll send a secure reset link."}
          </p>
        </div>

        <form className="password-recovery-form" onSubmit={submit} noValidate>
          {isReset ? (
            <>
              <label htmlFor="reset-password">New password</label>
              <input
                id="reset-password"
                className="password-recovery-input"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                minLength={12}
                maxLength={128}
                placeholder="At least 12 characters"
                required
              />
              <label htmlFor="reset-password-confirm">Confirm new password</label>
              <input
                id="reset-password-confirm"
                className="password-recovery-input"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(event) => setConfirmPassword(event.target.value)}
                minLength={12}
                maxLength={128}
                placeholder="Enter the same password again"
                required
              />
            </>
          ) : (
            <>
              <label htmlFor="reset-email">Email address</label>
              <input
                id="reset-email"
                className="password-recovery-input"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                maxLength={254}
                placeholder="you@example.com"
                required
              />
            </>
          )}

          {error && <p className="password-recovery-error" role="alert">{error}</p>}
          {status && <p className="password-recovery-status" role="status">{status}</p>}

          {isReset && status ? (
            <button
              className="primary-button password-recovery-submit"
              type="button"
              onClick={() => navigate("/login", { replace: true })}
            >
              Return to sign in <ArrowRight size={17} />
            </button>
          ) : (
            <button className="primary-button password-recovery-submit" type="submit" disabled={busy}>
              {busy ? "Please wait..." : isReset ? "Save new password" : "Send reset link"}
              <ArrowRight size={17} />
            </button>
          )}
        </form>

        <button className="password-recovery-back" type="button" onClick={() => navigate("/login")}>
          <ArrowLeft size={15} /> Back to sign in
        </button>

        <p className="auth-security password-recovery-security">
          <ShieldCheck size={14} /> Reset tokens are single-use, expire after 30 minutes, and are stored only as hashes.
        </p>
      </section>
    </main>
  );
}
