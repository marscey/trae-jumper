import { check, type Update } from "@tauri-apps/plugin-updater";

export interface UpdateInfo {
  update: Update;
  version: string;
  /** 本地化后的发布日期（无则为空串） */
  date: string;
}

/** 检查是否有新版本；无更新时返回 null */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  const update = await check();
  if (!update) return null;
  return {
    update,
    version: update.version,
    date: update.date ? new Date(update.date).toLocaleDateString("zh-CN") : "",
  };
}

/** 下载更新包，`onProgress` 回调下载百分比（0-100） */
export async function downloadUpdate(
  update: Update,
  onProgress?: (percent: number) => void
): Promise<void> {
  let downloaded = 0;
  let total = 0;
  await update.download((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? 0;
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        break;
    }
    if (total > 0 && onProgress) {
      onProgress(Math.min(100, Math.round((downloaded / total) * 100)));
    }
  });
}

/** 安装已下载的更新包（安装后通常需重启应用生效） */
export async function installUpdate(update: Update): Promise<void> {
  await update.install();
}
