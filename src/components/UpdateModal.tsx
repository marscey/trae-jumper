interface UpdateModalProps {
  isOpen: boolean;
  /** 版本号，如 "1.0.4" */
  version: string;
  /** 发布日期（本地化后） */
  date: string;
  /** Release notes */
  notes: string;

  /** 当前状态 */
  status: "available" | "downloading" | "ready";
  /** 下载百分比 0-100 */
  percent: number;

  /** 状态为 available 时：立即更新 */
  onUpdate: () => void;
  /** 状态为 available 时：跳过本次（记 localStorage） */
  onSkip: () => void;
  /** 状态为 ready 时：立即重启 */
  onRestart: () => void;
  /** 关闭（available 时 = 临时收起弹窗，下次轮询还会再出现） */
  onClose: () => void;
}

export function UpdateModal({
  isOpen,
  version,
  date,
  notes,
  status,
  percent,
  onUpdate,
  onSkip,
  onRestart,
  onClose,
}: UpdateModalProps) {
  if (!isOpen) return null;

  const hasNotes = notes && notes.trim().length > 0;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="info-modal update-modal" onClick={(e) => e.stopPropagation()}>
        <div className="info-modal-header">
          <div className="info-modal-icon">✨</div>
          <h3 className="info-modal-title">
            {status === "ready"
              ? `v${version} 更新完成`
              : status === "downloading"
              ? "正在下载更新"
              : `发现新版本 v${version}`}
          </h3>
          <button className="info-modal-close" onClick={onClose} title="关闭">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div className="info-modal-body update-modal-body">
          {status === "available" && (
            <>
              {date && (
                <p className="update-meta">发布日期：{date}</p>
              )}
              {hasNotes ? (
                <pre className="update-notes">{notes}</pre>
              ) : (
                <p className="update-meta">新版本已发布，点击下方按钮立即更新。</p>
              )}
            </>
          )}

          {status === "downloading" && (
            <div className="update-progress">
              <p className="update-meta">
                正在下载 v{version}… {percent > 0 ? `${percent}%` : ""}
              </p>
              <div className="update-progress-track">
                <div
                  className="update-progress-fill"
                  style={{ width: `${percent}%` }}
                />
              </div>
              {percent === 0 && (
                <p className="update-meta update-meta-hint">
                  正在连接服务器，请稍候…
                </p>
              )}
            </div>
          )}

          {status === "ready" && (
            <>
              <div className="update-ready-icon">✅</div>
              <p className="update-meta">
                v{version} 已下载并安装完成，重启 TraeJumper 后生效。
              </p>
            </>
          )}
        </div>

        <div className="info-modal-footer">
          {status === "available" && (
            <>
              <button className="info-btn extra" onClick={onSkip}>
                跳过本次
              </button>
              <button className="info-btn cancel" onClick={onClose}>
                稍后再说
              </button>
              <button className="info-btn confirm" onClick={onUpdate}>
                立即更新
              </button>
            </>
          )}
          {status === "downloading" && (
            <button className="info-btn confirm" disabled>
              下载中 {percent}%
            </button>
          )}
          {status === "ready" && (
            <>
              <button className="info-btn cancel" onClick={onClose}>
                稍后重启
              </button>
              <button className="info-btn confirm" onClick={onRestart}>
                立即重启
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
