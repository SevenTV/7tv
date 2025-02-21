<script lang="ts">
	import TagsInput from "../input/tags-input.svelte";
	import Dialog, { type DialogMode } from "./dialog.svelte";
	import TextInput from "../input/text-input.svelte";
	import Checkbox from "../input/checkbox.svelte";
	import Button from "../input/button.svelte";
	import { t } from "svelte-i18n";
	import type { Emote } from "$/gql/graphql";
	import { user } from "$/lib/auth";
	import Spinner from "../spinner.svelte";
	import { updateFlags, updateName, updateTags } from "$/lib/emoteMutations";
	import DeleteEmoteDialog from "./delete-emote-dialog.svelte";
	import { compareTags } from "$/lib/utils";

	interface Props {
		mode: DialogMode;
		data: Emote;
	}

	let { mode = $bindable("hidden"), data = $bindable() }: Props = $props();

	let name = $state(data.defaultName);
	let tags = $state(data.tags);

	let zeroWidth = $state(data.flags.defaultZeroWidth);
	let publicListed = $state(data.flags.publicListed);
	let approvedPersonal = $state(data.flags.approvedPersonal);
	let deniedPersonal = $state(data.flags.deniedPersonal);
	let privateFlag = $state(data.flags.private);
	let nsfw = $state(data.flags.nsfw);

	$effect(() => {
		if (approvedPersonal) {
			deniedPersonal = false;
		}
	});

	$effect(() => {
		if (deniedPersonal) {
			approvedPersonal = false;
		}
	});

	let nameChanged = $derived(name !== data.defaultName);
	let tagsChanged = $derived(!compareTags(tags, data.tags));
	let flagsChanged = $derived(
		zeroWidth !== data.flags.defaultZeroWidth ||
			publicListed !== data.flags.publicListed ||
			approvedPersonal !== data.flags.approvedPersonal ||
			deniedPersonal !== data.flags.deniedPersonal ||
			privateFlag !== data.flags.private ||
			nsfw !== data.flags.nsfw,
	);

	let loading = $state(false);

	async function submit() {
		loading = true;

		if (nameChanged) {
			// Update name
			const newData = await updateName(data.id, name);

			if (newData) {
				data = newData;
			}
		}

		if (tagsChanged) {
			// Update tags
			const newData = await updateTags(data.id, tags);

			if (newData) {
				data = newData;
			}
		}

		if (flagsChanged) {
			// Update flags
			const newFlags = {
				defaultZeroWidth: zeroWidth,
				publicListed,
				approvedPersonal,
				deniedPersonal,
				private: privateFlag,
				nsfw,
			};

			const newData = await updateFlags(data.id, newFlags);

			if (newData) {
				data = newData;
			}
		}

		loading = false;
		mode = "hidden";
	}

	let deleteDialogMode: DialogMode = $state("hidden");

	function showDeleteDialog() {
		deleteDialogMode = "shown";
		mode = "hidden";
	}

	function onModeChange(mode: string) {
		if (mode === "hidden") {
			name = data.defaultName;
			tags = data.tags;

			zeroWidth = data.flags.defaultZeroWidth;
			publicListed = data.flags.publicListed;
			approvedPersonal = data.flags.approvedPersonal;
			deniedPersonal = data.flags.deniedPersonal;
			privateFlag = data.flags.private;
			nsfw = data.flags.nsfw;
		}
	}

	$effect(() => {
		onModeChange(mode);
	});
</script>

<DeleteEmoteDialog bind:mode={deleteDialogMode} {data} />
<Dialog bind:mode>
	<form class="layout">
		<h1>{$t("dialogs.edit_emote.title", { values: { emote: data.defaultName } })}</h1>
		<hr />
		<TextInput placeholder={$t("labels.emote_name")} bind:value={name}>
			<span class="label">{$t("labels.emote_name")}</span>
		</TextInput>
		<div class="tags">
			<TagsInput bind:tags>
				<span class="label">{$t("labels.tags")}</span>
			</TagsInput>
		</div>
		<!-- <TextInput placeholder={$t("labels.search_users", { values: { count: 2 } })}>
			<span class="label">{$t("labels.emote_attribution")}</span>
			{#snippet icon()}
				<User />
			{/snippet}
		</TextInput> -->
		<div>
			<span class="label">{$t("common.settings")}</span>
			<div class="settings">
				<Checkbox bind:value={zeroWidth}>{$t("flags.overlaying")}</Checkbox>
				<Checkbox bind:value={privateFlag}>Private</Checkbox>
				{#if $user?.permissions.emote.manageAny}
					<Checkbox bind:value={publicListed}>{$t("flags.listed")}</Checkbox>
					<Checkbox bind:value={approvedPersonal}>Approved Personal Use</Checkbox>
					<Checkbox bind:value={deniedPersonal}>Denied Personal Use</Checkbox>
					<Checkbox bind:value={nsfw}>NSFW</Checkbox>
				{/if}
			</div>
		</div>
		<div class="buttons">
			{#if $user && (data.owner?.id === $user.id || $user.permissions.emote.manageAny)}
				<Button style="color: var(--danger); margin-right: auto;" onclick={showDeleteDialog}>
					{$t("labels.delete")}
				</Button>
			{/if}
			<Button secondary onclick={() => (mode = "hidden")}>{$t("labels.cancel")}</Button>
			{#snippet loadingSpinner()}
				<Spinner />
			{/snippet}
			<Button
				primary
				submit
				icon={loading ? loadingSpinner : undefined}
				disabled={(!nameChanged && !tagsChanged && !flagsChanged) || loading}
				onclick={submit}
			>
				{$t("labels.save")}
			</Button>
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

	.label {
		font-size: 0.875rem;
		font-weight: 500;
	}

	.tags {
		display: flex;
		flex-direction: column;
	}

	.settings {
		margin-top: 0.4rem;

		display: grid;
		grid-template-columns: auto auto;
		gap: 0.5rem;
	}

	.buttons {
		margin-top: auto;

		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
</style>
