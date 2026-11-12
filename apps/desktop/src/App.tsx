import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <main className="flex flex-col items-center gap-6 pt-[10vh] text-center">
      <h1 className="text-2xl font-semibold">Welcome to Tauri + React</h1>

      <div className="flex justify-center">
        <a href="https://vite.dev" target="_blank">
          <img
            src="/vite.svg"
            className="h-24 p-6 transition-[filter] duration-700 hover:drop-shadow-[0_0_2em_#747bff]"
            alt="Vite logo"
          />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img
            src="/tauri.svg"
            className="h-24 p-6 transition-[filter] duration-700 hover:drop-shadow-[0_0_2em_#24c8db]"
            alt="Tauri logo"
          />
        </a>
        <a href="https://react.dev" target="_blank">
          <img
            src={reactLogo}
            className="h-24 p-6 transition-[filter] duration-700 hover:drop-shadow-[0_0_2em_#61dafb]"
            alt="React logo"
          />
        </a>
      </div>
      <p className="text-muted-foreground">
        Click on the Tauri, Vite, and React logos to learn more.
      </p>

      <form
        className="flex justify-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          className="h-9 rounded-4xl border border-input bg-input/30 px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <Button type="submit">Greet</Button>
      </form>
      <p>{greetMsg}</p>
    </main>
  );
}

export default App;
