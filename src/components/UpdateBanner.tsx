interface UpdateBannerProps {
  status: "downloading" | "ready";
  version: string;
  /** 下载百分比（0-100，未知时为 0） */
  percent: number;
  onRestart: () => void;
  onDismiss: () => void;
}

export function UpdateBanner({
  status,
  version,
  percent,
  onRestart,
  onDismiss,
}: UpdateBannerProps) {
  return (
    <div className="update-banner">
      {(status === "downloading") ? (
        <>
          <span className="update-banner-spinner" />
          <span className="update-banner-text">
            发现新版本 v{version}，正在后台下载{percent > 0 ? ` ${percent}%` : "..."}
          </span>
          <div className="update-banner-track">
            <div className="update-banner-fill" style={{ width: `${percent}%` }} />
          </div>
        </>
      ) : (
        <>
          <span className="update-banner-text">
            新版本 v{version} 已下载完成，重启应用后生效
          </span>
          <button className="update-banner-btn" onClick={onRestart}>
            立即重启
          </button>
          <button className="update-banner-close" onClick={onDismiss} title="稍后重启">
            ×
          </button>
        </>
      )}
    </div>
  );
}
