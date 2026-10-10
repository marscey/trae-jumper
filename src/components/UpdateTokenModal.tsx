import { useEffect, useState } from "react";
import * as api from "../api";

interface UpdateTokenModalProps {
  isOpen: boolean;
  accountId: string;
  accountName: string;
  onClose: () => void;
  onUpdate: (accountId: string, token: string) => Promise<void>;
  /** 当前目标客户端名称（用于"从客户端读取"文案与账号一致性展示） */
  clientName?: string;
  /** 自动模式（客户端读取 / 浏览器登录）更新成功后回调（App 刷新数据） */
  onSuccess?: () => void;
  /** Toast 回调，用于显示错误/成功消息 */
  onToast?: (type: "success" | "error" | "warning" | "info", message: string) => void;
}

type UpdateMode = "client" | "browser" | "manual";
// 浏览器登录模式下，webview 当前登录账号与目标账号的一致性探测状态
type ProbeStatus = "idle" | "loading" | "match" | "mismatch" | "none" | "error";

export function UpdateTokenModal({
  isOpen,
  accountId,
  accountName,
  onClose,
  onUpdate,
  onSuccess,
  clientName,
  onToast,
}: UpdateTokenModalProps) {
  const [mode, setMode] = useState<UpdateMode>("client");
  const [inputValue, setInputValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [browserStarted, setBrowserStarted] = useState(false);
  // "从客户端读取"模式下，客户端当前登录账号与目标账号的一致性检测
  const [clientLogin, setClientLogin] = useState<{ user_id: string; email?: string } | null>(null);
  const [clientChecked, setClientChecked] = useState(false);
  const [targetUser, setTargetUser] = useState<{ user_id?: string } | null>(null);
  // "浏览器登录"模式下，webview 当前登录账号与目标账号的一致性检测
  const [probeStatus, setProbeStatus] = useState<ProbeStatus>("idle");
  const [probeResult, setProbeResult] = useState<{ user_id: string; email?: string } | null>(null);

  // 进入"从客户端读取"模式时，读取并比对客户端当前登录账号与目标账号
  useEffect(() => {
    if (!isOpen || mode !== "client") return;
    if (!api.hasTauri()) return;
    let cancelled = false;
    setClientChecked(false);
    setClientLogin(null);
    setTargetUser(null);
    (async () => {
      const [login, tgt] = await Promise.all([
        api.currentClientLogin().catch(() => null),
        api.getAccount(accountId).catch(() => null),
      ]);
      if (cancelled) return;
      setClientLogin(login);
      setTargetUser(tgt ? { user_id: tgt.user_id } : null);
      setClientChecked(true);
    })();
    return () => {
      cancelled = true;
    };
  }, [isOpen, mode, accountId]);

  // 进入"浏览器登录"模式时，拉取目标账号的 user_id 用于 probe 一致性比对
  useEffect(() => {
    if (!isOpen || mode !== "browser") return;
    if (!api.hasTauri()) return;
    let cancelled = false;
    (async () => {
      const tgt = await api.getAccount(accountId).catch(() => null);
      if (cancelled) return;
      setTargetUser(tgt ? { user_id: tgt.user_id } : null);
    })();
    return () => {
      cancelled = true;
    };
  }, [isOpen, mode, accountId]);

  // 浏览器登录打开期间监听主窗口 resize，让子 webview 跟随尺寸变化
  useEffect(() => {
    if (!api.hasTauri() || !browserStarted) return;
    let lastCall = 0;
    const onResize = () => {
      const now = Date.now();
      if (now - lastCall < 100) return; // 100ms 节流
      lastCall = now;
      api.resizeLoginChildWebview().catch(() => {});
    };
    window.addEventListener("resize", onResize);
    onResize();
    return () => window.removeEventListener("resize", onResize);
  }, [browserStarted]);

  if (!isOpen) return null;

  // 从输入中提取 Token
  const extractToken = (input: string): string | null => {
    const trimmed = input.trim();

    // 情况1: 直接是 JWT Token (以 eyJ 开头)
    if (trimmed.startsWith("eyJ")) {
      return trimmed;
    }

    // 情况2: 是 JSON 响应，尝试解析
    try {
      const json = JSON.parse(trimmed);

      // GetUserToken 接口的响应格式
      if (json.Result?.Token) {
        return json.Result.Token;
      }

      // 可能是其他格式
      if (json.token) {
        return json.token;
      }
      if (json.Token) {
        return json.Token;
      }
    } catch {
      // 不是有效的 JSON，继续尝试其他方式
    }

    // 情况3: 尝试用正则提取 Token
    const tokenMatch = trimmed.match(/"Token"\s*:\s*"(eyJ[^"]+)"/);
    if (tokenMatch) {
      return tokenMatch[1];
    }

    // 情况4: 尝试提取任何 eyJ 开头的字符串
    const jwtMatch = trimmed.match(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/);
    if (jwtMatch) {
      return jwtMatch[0];
    }

    return null;
  };

  const resetAll = () => {
    setError("");
    setInputValue("");
    setBrowserStarted(false);
    setMode("client");
    setClientLogin(null);
    setClientChecked(false);
    setTargetUser(null);
    setProbeStatus("idle");
    setProbeResult(null);
  };

  const handleCloseInternal = () => {
    // 若登录窗口仍开着，关闭它（fire-and-forget）
    if (browserStarted) {
      api.closeLoginWebview().catch(() => {});
    }
    resetAll();
    onClose();
  };

  // 手动粘贴更新
  const handleManualSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputValue.trim()) {
      setError("请输入新的 Token");
      return;
    }

    setLoading(true);
    setError("");

    try {
      const token = extractToken(inputValue);

      if (!token) {
        setError("无法识别 Token，请确保输入正确的 Token 或 GetUserToken 接口响应");
        setLoading(false);
        return;
      }

      await onUpdate(accountId, token);
      setInputValue("");
      onClose();
    } catch (err: any) {
      setError(err.message || "更新 Token 失败");
    } finally {
      setLoading(false);
    }
  };

  // 从当前 Trae 客户端读取并更新
  const handleFromClient = async () => {
    setLoading(true);
    setError("");
    try {
      await api.updateAccountTokenFromClient(accountId);
      onSuccess?.();
      handleCloseInternal();
    } catch (err: any) {
      setError(err.message || "从客户端读取 Token 失败");
    } finally {
      setLoading(false);
    }
  };

  // 打开浏览器登录窗口（update 模式：不自动检测，由用户手动刷新/确认）
  const handleBrowserLogin = async () => {
    setLoading(true);
    setError("");
    setProbeStatus("idle");
    setProbeResult(null);
    try {
      await api.startBrowserLoginForUpdate(accountId);
      setBrowserStarted(true);
    } catch (err: any) {
      setError(err.message || "打开登录窗口失败");
      setBrowserStarted(false);
    } finally {
      setLoading(false);
    }
  };

  // 刷新：探测登录窗口当前账号并与目标账号比对（用户手动触发）
  const handleRefresh = async () => {
    if (!browserStarted) return;
    setLoading(true);
    setError("");
    setProbeStatus("loading");
    try {
      const r = await api.probeLoginWebview();
      if (!r) {
        setProbeStatus("none");
        setProbeResult(null);
        return;
      }
      setProbeResult({ user_id: r.user_id, email: r.email });
      if (targetUser?.user_id && r.user_id === targetUser.user_id) {
        setProbeStatus("match");
      } else {
        setProbeStatus("mismatch");
      }
    } catch (err: any) {
      setProbeStatus("error");
      setError(err.message || "探测失败");
    } finally {
      setLoading(false);
    }
  };

  // 确认：应用登录窗口当前 Token 到目标账号（前置 match 后才可点）
  const handleConfirm = async () => {
    if (probeStatus !== "match") return;
    setLoading(true);
    setError("");
    try {
      await api.applyLoginWebviewToken(accountId);
      onSuccess?.();
      handleCloseInternal();
    } catch (err: any) {
      const msg = err.message || "更新失败";
      setError(msg);
      setProbeStatus("mismatch");
      onToast?.("error", msg);
    } finally {
      setLoading(false);
    }
  };

  // 客户端名称与账号一致性状态
  const displayName = clientName || "Trae 客户端";
  let clientStatus: "loading" | "match" | "mismatch" | "none" = "loading";
  if (clientChecked) {
    const curUserId = clientLogin?.user_id;
    if (!curUserId) {
      clientStatus = "none";
    } else if (targetUser?.user_id && curUserId === targetUser.user_id) {
      clientStatus = "match";
    } else {
      clientStatus = "mismatch";
    }
  }

  return (
    <div className="modal-overlay" onClick={handleCloseInternal}>
      <div
        className={`modal-content ${mode === "browser" ? "modal-content-fullscreen" : "add-account-modal"}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header-fixed">
          <h2>更新 Token</h2>
          <button className="modal-close-btn" onClick={handleCloseInternal}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
        </div>

        <div className="modal-body-scrollable">
          {/* 更新方式选择 */}
          <div className="add-mode-tabs">
            <button
              className={`mode-tab ${mode === "client" ? "active" : ""}`}
              onClick={() => setMode("client")}
              disabled={loading}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
              </svg>
              从客户端读取
            </button>
            <button
              className={`mode-tab ${mode === "browser" ? "active" : ""}`}
              onClick={() => setMode("browser")}
              disabled={loading}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/>
                <line x1="2" y1="12" x2="22" y2="12"/>
                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
              </svg>
              浏览器登录
            </button>
            <button
              className={`mode-tab ${mode === "manual" ? "active" : ""}`}
              onClick={() => setMode("manual")}
              disabled={loading}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
              </svg>
              手动输入
            </button>
          </div>

          <div className="token-help">
            <p className="modal-desc">
              为账号 <strong>{accountName}</strong> 更新 Token。
              <br />
              <small>三种方式都会校验新 Token 与当前账号是同一用户，否则无法更新。</small>
            </p>
          </div>

          {mode === "client" ? (
            /* 从 Trae 客户端读取 */
            <div className="trae-ide-mode">
              <div className="mode-description-simple">
                <div className="mode-icon">
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
                    <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
                    <line x1="12" y1="22.08" x2="12" y2="12"/>
                  </svg>
                </div>
                <h3>从 {displayName} 自动获取并更新</h3>
                <p>请先在 {displayName} 中登录「{accountName}」，系统将自动读取登录态并更新本条账号的 Token</p>

                {/* 客户端当前登录账号与目标账号的一致性提示 */}
                {clientStatus === "loading" && (
                  <p style={{ color: "var(--color-text-dim, #888)", marginTop: "8px" }}>
                    正在检测 {displayName} 当前登录账号...
                  </p>
                )}
                {clientStatus === "match" && (
                  <p style={{ color: "var(--color-success, #2f9e44)", marginTop: "8px" }}>
                    当前 {displayName} 已登录：「{clientLogin?.email || "(未显示邮箱)"}」
                    <br />
                    <small>与目标账号一致，可直接更新</small>
                  </p>
                )}
                {clientStatus === "none" && (
                  <p style={{ color: "var(--color-warning, #f0a030)", marginTop: "8px" }}>
                    未检测到 {displayName} 的登录账号，请先在 {displayName} 中登录「{accountName}」后再更新。
                  </p>
                )}
                {clientStatus === "mismatch" && (
                  <p
                    style={{
                      color: "#e03131",
                      marginTop: "8px",
                      fontWeight: 600,
                      background: "rgba(224, 49, 49, 0.08)",
                      padding: "8px 10px",
                      borderRadius: "8px",
                    }}
                  >
                    检测到 {displayName} 当前登录的账号
                    「{clientLogin?.email || "(未显示邮箱)"}」与要更新的账号「{accountName}」不一致，暂无法更新。
                    <br />
                    <small style={{ fontWeight: 400 }}>
                      请先在 {displayName} 中切换到「{accountName}」，检测会自动刷新后再更新。
                    </small>
                  </p>
                )}
              </div>
              {error && <div className="error-message">{error}</div>}
            </div>
          ) : mode === "browser" ? (
            /* 浏览器登录（手动确认）：开窗后中部由子 webview 占据，React 留空 */
            !browserStarted ? (
              <div className="trae-ide-mode">
                <div className="mode-description-simple">
                  <div className="mode-icon">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                      <circle cx="12" cy="12" r="10"/>
                      <line x1="2" y1="12" x2="22" y2="12"/>
                      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
                    </svg>
                  </div>
                  <h3>浏览器登录并手动确认</h3>
                  <p>点击下方「打开登录窗口」，在其中登录「{accountName}」后，回到本弹窗点「刷新」探测账号，一致后点「确认」导入。</p>
                </div>
                {error && <div className="error-message">{error}</div>}
              </div>
            ) : (
              /* browserStarted：中部留空，子 webview（Rust 创建）占据此区域 */
              <div className="modal-body-webview-placeholder" />
            )
          ) : (
            /* 手动粘贴 */
            <form onSubmit={handleManualSubmit}>
              <div className="token-help">
                <details>
                  <summary>如何获取新 Token？</summary>
                  <ol>
                    <li>打开 <a href="https://www.trae.ai/account-setting#usage" target="_blank" rel="noopener noreferrer">trae.ai 账号设置页面</a> 并登录对应账号</li>
                    <li>按 <kbd>F12</kbd> 打开开发者工具</li>
                    <li>切换到 <strong>Network</strong> 标签</li>
                    <li>刷新页面，在请求列表中找到 <code>GetUserToken</code></li>
                    <li>点击该请求，在右侧 <strong>Response</strong> 标签中复制整个响应内容，粘贴到下方</li>
                  </ol>
                </details>
              </div>
              <textarea
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder='粘贴新的 Token 或 API 响应...'
                rows={8}
                disabled={loading}
              />
              {error && <div className="error-message">{error}</div>}
            </form>
          )}

          {mode === "manual" && error && (
            <div className="error-message">{error}</div>
          )}
        </div>

        <div className="modal-actions-fixed">
          {mode === "browser" && browserStarted ? (
            /* browser 模式开窗后：probe 状态行 + 按钮行（两行布局，子 webview 不遮挡此区域） */
            <div className="browser-actions-container">
              <div
                className="probe-status-line"
                style={{
                  color:
                    probeStatus === "match" ? "var(--color-success, #2f9e44)" :
                    probeStatus === "mismatch" || probeStatus === "error" ? "#e03131" :
                    probeStatus === "none" ? "var(--color-warning, #f0a030)" :
                    "var(--color-text-dim, #888)",
                }}
              >
                {probeStatus === "loading" ? "正在探测登录账号..." :
                  probeStatus === "match" ? `已登录：${probeResult?.email || "(未显示邮箱)"}，可点确认导入` :
                  probeStatus === "mismatch" ? `账号不一致：${probeResult?.email || "(未显示邮箱)"}，请在登录窗口切换后刷新` :
                  probeStatus === "none" ? "未探测到登录态，请完成登录后点刷新" :
                  probeStatus === "error" ? (error || "探测失败，请重试刷新") :
                  "请在登录窗口完成登录后点刷新探测账号"}
              </div>
              <div className="modal-actions-row">
                <button type="button" onClick={handleCloseInternal} disabled={loading}>
                  取消
                </button>
                <button type="button" onClick={handleRefresh} disabled={loading}>
                  刷新
                </button>
                <button
                  type="button"
                  className="primary"
                  onClick={handleConfirm}
                  disabled={loading || probeStatus !== "match"}
                  title={probeStatus !== "match" ? "登录账号与目标账号不一致，请先切换或刷新" : undefined}
                >
                  {probeStatus === "match" ? "确认导入" : "账号不一致"}
                </button>
              </div>
            </div>
          ) : (
            <>
              <button type="button" onClick={handleCloseInternal} disabled={loading}>
                取消
              </button>
              {mode === "client" ? (
                <button
                  type="button"
                  className="primary"
                  onClick={handleFromClient}
                  disabled={loading || clientStatus === "mismatch"}
                  title={clientStatus === "mismatch" ? "当前客户端登录账号与目标账号不一致，请先切换账号" : undefined}
                >
                  {clientStatus === "mismatch"
                    ? "请先切换客户端账号"
                    : loading
                    ? "读取更新中..."
                    : "从客户端读取并更新"}
                </button>
              ) : mode === "browser" ? (
                <button
                  type="button"
                  className="primary"
                  onClick={handleBrowserLogin}
                  disabled={loading}
                >
                  {loading ? "打开中..." : "打开登录窗口"}
                </button>
              ) : (
                <button
                  type="button"
                  className="primary"
                  onClick={() => handleManualSubmit({} as React.FormEvent)}
                  disabled={loading}
                >
                  {loading ? "更新中..." : "更新 Token"}
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default UpdateTokenModal;