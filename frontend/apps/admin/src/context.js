import { createContext, useContext } from "react";

/** 운영자 화면 공통 값: me, toast(), go(tab, params), counts, refreshCounts() */
export const AdminContext = createContext(null);
export const useAdmin = () => useContext(AdminContext);
