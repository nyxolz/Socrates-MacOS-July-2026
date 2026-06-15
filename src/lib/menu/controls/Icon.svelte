<script lang="ts" context="module">
    // Minimalist 14×14 line icons. Add new ones here by `name → path d`.
    // viewBox is 24 so existing Feather/Lucide-style paths drop in clean.
    export const ICONS: Record<string, string> = {
        target:
            "M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0 M12 12m-5 0a5 5 0 1 0 10 0a5 5 0 1 0 -10 0 M12 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M3 12h2 M19 12h2 M12 3v2 M12 19v2",
        shield:
            "M12 3l8 3v6c0 4.5-3 8.5-8 9-5-.5-8-4.5-8-9V6l8-3z",
        eye:
            "M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
        text:
            "M4 6h16 M4 12h10 M4 18h16",
        filter:
            "M3 5h18 M6 12h12 M10 19h4",
        zap:
            "M13 2L4 14h7l-1 8 9-12h-7l1-8z",
        list:
            "M3 6h18 M3 12h18 M3 18h18 M3 6m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M3 12m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0 M3 18m-1 0a1 1 0 1 0 2 0a1 1 0 1 0 -2 0",
        server:
            "M3 4h18v6H3z M3 14h18v6H3z M7 7h.01 M7 17h.01",
        code:
            "M9 6l-6 6 6 6 M15 6l6 6-6 6",
        package:
            "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z M3.27 6.96L12 12.01l8.73-5.05 M12 22.08V12",
        terminal:
            "M4 17l6-6-6-6 M12 19h8",
        sliders:
            "M4 6h13 M19 6h1 M4 12h1 M7 12h13 M4 18h9 M15 18h5 M17 6m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0 M6 12m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0 M14 18m-2 0a2 2 0 1 0 4 0a2 2 0 1 0 -4 0",
        users:
            "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75",
        user:
            "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8",
        x:
            "M6 6l12 12 M18 6L6 18",
        minus:
            "M5 12h14",
        plus:
            "M5 12h14 M12 5v14",
        crosshair:
            "M12 12m-10 0a10 10 0 1 0 20 0a10 10 0 1 0 -20 0 M22 12h-4 M6 12H2 M12 6V2 M12 22v-4",
        mouse:
            "M3 3l7 19 2-8 8-2-17-9z",
        activity:
            "M22 12h-4l-3 9L9 3l-3 9H2",
        compass:
            "M12 12m-10 0a10 10 0 1 0 20 0a10 10 0 1 0 -20 0 M16.24 7.76L14.12 14.12 7.76 16.24 9.88 9.88z",
        check:
            "M5 12l5 5L20 7",
        gear:
            "M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0 M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z",
        trash:
            "M3 6h18 M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6 M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2 M10 11v6 M14 11v6",
        edit:
            "M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z",
        load:
            "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3",
        file:
            "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6",
        calendar:
            "M3 4h18v17a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z M16 2v4 M8 2v4 M3 10h18",
        copy:
            "M9 9h10v10H9z M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1",
    };
</script>

<script lang="ts">
    export let name: keyof typeof ICONS | string;
    export let size: number = 12;
</script>

<svg
    class="icon"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
>
    <path d={ICONS[name] ?? ""} />
</svg>

<style>
    .icon {
        display: inline-block;
        flex-shrink: 0;
    }
</style>
