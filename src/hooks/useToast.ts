import { useState, useCallback } from 'react';

export interface ToastMessage {
  id: string;
  type: 'success' | 'error' | 'warning' | 'info';
  message: string;
  duration?: number;
}

/** Toast 类型（供各 hooks 参数签名共用） */
export type ToastType = ToastMessage['type'];

/** 各业务 hooks 通用的 addToast 函数签名 */
export type AddToastFn = (type: ToastType, message: string, duration?: number) => void;

export function useToast() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const addToast = useCallback((
    type: ToastMessage['type'],
    message: string,
    duration?: number
  ) => {
    const id = Date.now().toString();
    setToasts((prev) => [...prev, { id, type, message, duration }]);
  }, []);

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
