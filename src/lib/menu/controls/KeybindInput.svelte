<script lang="ts">
    import { onDestroy } from "svelte";

    export let label: string | undefined = undefined;
    export let value: string;
    export let inline: boolean = false;

    let listening = false;

    const MOUSE_BUTTONS: Record<number, string> = {
        0: "MouseLeft",
        1: "MouseMiddle",
        2: "MouseRight",
        3: "Mouse4",
        4: "Mouse5",
    };

    function startListen(e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        listening = true;
        setTimeout(attach, 0);
    }

    function normalizeKey(e: KeyboardEvent): string {
        return e.code || e.key;
    }

    function onKey(e: KeyboardEvent) {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") { stopListen(); return; }
        value = normalizeKey(e);
        stopListen();
    }

    function onMouse(e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        value = MOUSE_BUTTONS[e.button] ?? `Mouse${e.button}`;
        stopListen();
    }

    function attach() {
        window.addEventListener("keydown", onKey, true);
        window.addEventListener("mousedown", onMouse, true);
    }
    function detach() {
        window.removeEventListener("keydown", onKey, true);
        window.removeEventListener("mousedown", onMouse, true);
    }
    function stopListen() {
        detach();
        listening = false;
    }

    onDestroy(detach);
</script>

{#if inline}
    <button
        class="bind compact"
        class:listening
        on:mousedown={startListen}
        title="Click then press any key or mouse button (Esc to cancel)"
    >
        {listening ? "..." : value || "bind"}
    </button>
{:else}
    <div class="kb">
        {#if label}<span class="label">{label}</span>{/if}
        <button
            class="bind"
            class:listening
            on:mousedown={startListen}
            title="Click then press any key or mouse button (Esc to cancel)"
        >
            {listening ? "press a key..." : value || "click to bind"}
        </button>
    </div>
{/if}

<style>
    .kb {
        display: flex;
        flex-direction: column;
        gap: 3px;
        padding: 2px 0;
    }
    .label {
        font-size: 11px;
        color: var(--text);
    }
    .bind {
        background: var(--bg-input);
        border: 1px solid var(--border);
        border-radius: var(--radius-small);
        color: var(--text);
        font: inherit;
        font-size: 11px;
        padding: 5px 8px;
        text-align: left;
        cursor: pointer;
        outline: none;
        font-variant-numeric: tabular-nums;
    }
    .bind {
        transition: border-color 140ms var(--ease), background 140ms var(--ease), color 140ms var(--ease);
    }
    .bind:hover {
        border-color: var(--accent);
    }
    .bind.listening {
        border-color: var(--accent);
        color: var(--accent);
        background: rgba(255, 222, 157, 0.07);
    }
    .bind.compact {
        padding: 3px 7px;
        font-size: 10px;
        min-width: 64px;
        text-align: center;
        margin-left: auto;
    }
</style>
