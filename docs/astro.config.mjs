// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// Сайт публикуется на GitHub Pages в /hostsctl/, поэтому `base` обязан совпасть
// с именем репозитория — иначе локально всё работает, а на деплое каждая
// внутренняя ссылка отдаёт 404.
export default defineConfig({
  site: 'https://jtprogru.github.io',
  base: '/hostsctl',
  integrations: [
    starlight({
      title: 'hostsctl',
      description:
        'Manage /etc/hosts from a YAML config: groups, zone files, remote blocklists, backups.',
      logo: { src: './src/assets/logo.svg', replacesTitle: false },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/jtprogru/hostsctl' },
      ],
      editLink: {
        baseUrl: 'https://github.com/jtprogru/hostsctl/edit/main/docs/',
      },
      // Английский — основной язык. Русские страницы, которых ещё нет, падают
      // на английский оригинал, а не в 404, поэтому неполная локаль допустима.
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        ru: { label: 'Русский', lang: 'ru' },
      },
      sidebar: [
        {
          label: 'Start here',
          translations: { ru: 'Начало' },
          items: [{ slug: 'getting-started' }, { slug: 'install' }],
        },
        {
          label: 'Guides',
          translations: { ru: 'Руководства' },
          items: [
            { slug: 'guides/configuration' },
            { slug: 'guides/groups' },
            { slug: 'guides/zones' },
            { slug: 'guides/blocklists' },
            { slug: 'guides/backups' },
            { slug: 'guides/permissions' },
          ],
        },
        {
          label: 'Reference',
          translations: { ru: 'Справочник' },
          items: [
            { slug: 'reference/cli' },
            { slug: 'reference/config' },
            { slug: 'reference/exit-codes' },
          ],
        },
        {
          label: 'Project',
          translations: { ru: 'Проект' },
          items: [
            { slug: 'project/how-it-works' },
            { slug: 'project/migration' },
            { slug: 'project/contributing' },
          ],
        },
      ],
      lastUpdated: true,
      credits: false,
    }),
  ],
});
