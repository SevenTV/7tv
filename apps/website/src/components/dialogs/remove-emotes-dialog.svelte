<script lang="ts">
	import Button from "../input/button.svelte";
	import Dialog, { type DialogMode } from "./dialog.svelte";
	import { t } from "svelte-i18n";

	let { mode = $bindable("hidden") }: { mode: DialogMode } = $props();
</script>

<Dialog width={35} bind:mode>
	<form class="layout">
		<h1>{$t("dialogs.remove_emote.title", { values: { set: "Cat Emotes" } })}</h1>
		<hr />
		<!-- <EmoteContainer layout={"small-grid"} style="max-height: 11rem" scrollable>
			{#each Array(100) as _, i}
				<EmotePreview index={i} emoteOnly />
			{/each}
		</EmoteContainer> -->
		<div class="buttons">
			<Button style="color: var(--danger)" submit>{$t("labels.remove")}</Button>
			<Button secondary onclick={() => (mode = "hidden")}>{$t("labels.cancel")}</Button>
		</div>
	</form>
</Dialog>

<style lang="scss">
	.layout {
		padding: 1rem;

		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	h1 {
		font-size: 1rem;
		font-weight: 600;
	}

	.buttons {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 0.5rem;
	}
</style>
