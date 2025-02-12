<script lang="ts" module>
	export const TAGS_SEPERATOR = " ";
</script>

<script lang="ts">
	import Expandable from "$/components/expandable.svelte";
	import Checkbox from "$/components/input/checkbox.svelte";
	import { Smiley, MagnifyingGlass } from "phosphor-svelte";
	import TagsInput from "$/components/input/tags-input.svelte";
	import TabLink from "$/components/tab-link.svelte";
	import TextInput from "$/components/input/text-input.svelte";
	import { t } from "svelte-i18n";
	import { page } from "$app/stores";
	import { type Page } from "@sveltejs/kit";
	import { goto } from "$app/navigation";
	import type { Snippet } from "svelte";

	let { children }: { children: Snippet } = $props();

	let query: string | undefined = $state($page.url.searchParams.get("q") ?? undefined);
	let tags: string[] = $state($page.url.searchParams.get("t")?.split(TAGS_SEPERATOR) ?? []);
	let animated: boolean = $state($page.url.searchParams.get("a") == "1");
	let staticFilter: boolean = $state($page.url.searchParams.get("s") == "1");
	let overlaying: boolean = $state($page.url.searchParams.get("o") == "1");
	let personalUse: boolean = $state($page.url.searchParams.get("p") == "1");
	let exactMatch: boolean = $state($page.url.searchParams.get("e") == "1");

	function unsetStaticFilter() {
		staticFilter = false;
	}

	function unsetAnimated() {
		animated = false;
	}

	$effect(() => {
		if (animated) {
			unsetStaticFilter();
		}
	});

	$effect(() => {
		if (staticFilter) {
			unsetAnimated();
		}
	});

	$effect(() => {
		let url = new URL($page.url);

		// Needed for update when on the same page
		if ($page.url.searchParams.get("updateSearch") === "true") {
			query = $page.url.searchParams.get("q") ?? undefined;
			url.searchParams.delete("updateSearch");
		}

		if (query) {
			url.searchParams.set("q", query);
		} else {
			url.searchParams.delete("q");
		}

		if (tags && tags.length > 0) {
			url.searchParams.set("t", tags.join(TAGS_SEPERATOR));
		} else {
			url.searchParams.delete("t");
		}

		if (animated) {
			url.searchParams.set("a", "1");
		} else {
			url.searchParams.delete("a");
		}

		if (staticFilter) {
			url.searchParams.set("s", "1");
		} else {
			url.searchParams.delete("s");
		}

		if (overlaying) {
			url.searchParams.set("o", "1");
		} else {
			url.searchParams.delete("o");
		}

		if (personalUse) {
			url.searchParams.set("p", "1");
		} else {
			url.searchParams.delete("p");
		}

		if (exactMatch) {
			url.searchParams.set("e", "1");
		} else {
			url.searchParams.delete("e");
		}

		goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	});

	function menuMatcher(page: Page, href: string | undefined) {
		switch (href) {
			case "/emotes":
				return page.route.id?.startsWith("/emotes/(directory)/(emotes)") ?? false;
			case "/emotes/sets":
				return page.route.id?.startsWith("/emotes/(directory)/sets") ?? false;
			case "/emotes/bookmarked":
				return page.route.id?.startsWith("/emotes/(directory)/bookmarked") ?? false;
		}
		return false;
	}
</script>

<div class="side-bar-layout">
	<aside class="side-bar">
		<h1>{$t("pages.directory.title")}</h1>
		<nav class="link-list">
			<TabLink
				href="/emotes"
				title={$t("common.emotes", { values: { count: 2 } })}
				big
				matcher={menuMatcher}
			>
				<Smiley />
				{#snippet active()}
					<Smiley weight="fill" />
				{/snippet}
			</TabLink>
		</nav>
		<hr />
		<Expandable title={$t("labels.search")} expanded={true}>
			<TextInput placeholder={$t("common.emotes", { values: { count: 1 } })} bind:value={query}>
				{#snippet icon()}
					<MagnifyingGlass />
				{/snippet}
			</TextInput>
		</Expandable>
		<Expandable title={$t("labels.tags")}>
			<TagsInput bind:tags />
		</Expandable>
		<Expandable title={$t("labels.filters")}>
			<div class="filters">
				<Checkbox bind:value={animated}>{$t("pages.directory.filters.animated")}</Checkbox>
				<Checkbox bind:value={staticFilter}>{$t("pages.directory.filters.static")}</Checkbox>
				<Checkbox bind:value={overlaying}>{$t("flags.overlaying")}</Checkbox>
				<Checkbox bind:value={personalUse}>Personal Use</Checkbox>
				<Checkbox bind:value={exactMatch}>{$t("pages.directory.filters.exact_match")}</Checkbox>
			</div>
		</Expandable>
	</aside>
	<div class="content">
		{@render children()}
	</div>
</div>

<style lang="scss">
	.filters {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
</style>
