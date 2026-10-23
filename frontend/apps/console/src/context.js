import { createContext, useContext } from "react";

/** 파트너 화면 공통 값: me(업체·계정), reload(), toast(), go(tab) */
export const PartnerContext = createContext(null);
export const usePartner = () => useContext(PartnerContext);
