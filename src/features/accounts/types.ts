import type { Dispatch, SetStateAction } from "react";

/** React setState 的通用签名（供各业务 hooks 参数复用） */
export type SetStateFn<T> = Dispatch<SetStateAction<T>>;
