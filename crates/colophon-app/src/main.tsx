import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Frontiere } from "./Frontiere";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Frontiere>
      <App />
    </Frontiere>
  </React.StrictMode>,
);
