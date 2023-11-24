import RemoveIcon from '@mui/icons-material/Remove';
import { Box, BoxProps, Stack, Typography } from '@mui/material';
import { styled } from '@mui/material/styles';
import {
  IconDatabaseCog,
  IconDatabasePlus,
  IconFolderPlus,
  IconRefresh,
} from '@tabler/icons-react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/primitives';
import * as dialog from '@tauri-apps/plugin-dialog';
import { useEffect } from 'react';

import { getDB } from '@/api';
import { useDBConfigStore } from '@/components/DBConfig';
import { MuiIconButton } from '@/components/MuiIconButton';
import ToggleColorMode from '@/components/ToggleColorMode';
import Setting from '@/pages/Setting';
import { DTableType } from '@/stores/dataset';
import { DialectType, useDBListStore } from '@/stores/dbList';
import { borderTheme } from '@/utils';

const ToolbarBox = styled(Box)<BoxProps>(({ theme }) => ({
  height: 32,
  width: '100%',
  paddingLeft: '1rem',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  borderBottom: borderTheme(theme),
}));

export function SideToolbar({
  selectedTable,
}: {
  selectedTable: DTableType | null;
}) {
  const dbList = useDBListStore((state) => state.dbList);
  const appendDB = useDBListStore((state) => state.append);
  const removeDB = useDBListStore((state) => state.remove);
  const updateDB = useDBListStore((state) => state.update);

  async function handleGetDB(url: string, dialect: DialectType) {
    const data = await getDB({ url, dialect });
    appendDB(data);
  }

  async function openUrl() {
    const path: string = await invoke('opened_urls');
    console.log(path);
  }

  useEffect(() => {
    openUrl();
    const unlisten = listen('open-directory', (e) => {
      console.log(e.payload);

      // TODO: open data file
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const onOpen = useDBConfigStore((state) => state.onOpen);
  const setDB = useDBConfigStore((state) => state.setDB);

  const handleOpen = () => {
    setDB(selectedTable!);
    onOpen();
  };

  async function handleRemoveDB() {
    if (selectedTable) {
      removeDB(selectedTable.root);
    }
  }
  async function handleAppendDB() {
    const res = await dialog.open({
      directory: false,
      filters: [
        {
          name: 'Data File',
          extensions: ['duckdb', 'parquet', 'csv'],
        },
      ],
    });
    if (!res) {
      return;
    }
    if (res.path.endsWith('.duckdb')) {
      handleGetDB(res.path, 'duckdb');
    } else {
      handleGetDB(res.path, 'file');
    }
  }

  async function handleAppendFolder() {
    const res = await dialog.open({
      directory: true,
    });
    if (res) {
      handleGetDB(res, 'folder');
    }
  }

  async function handleRefresh() {
    if (selectedTable) {
      const root = selectedTable.root;

      dbList.forEach(async (db) => {
        if (db.id == root) {
          const { data } = await getDB({
            url: db.data.path,
            dialect: db.dialect,
          });
          updateDB(root, data);
        }
      });
    }
  }

  const isRoot = selectedTable
    ? dbList.map((item) => item.data.path).includes(selectedTable?.tableName)
    : false;

  return (
    <>
      <ToolbarBox>
        <Typography fontWeight={800}>Database Explorer</Typography>
        <Stack direction="row">
          <ToggleColorMode />
          <Setting />
        </Stack>
      </ToolbarBox>
      <ToolbarBox>
        <Stack
          direction="row"
          sx={{
            marginLeft: -1,
          }}
        >
          <MuiIconButton onClick={handleAppendFolder}>
            <IconFolderPlus />
          </MuiIconButton>
          <MuiIconButton onClick={handleAppendDB}>
            <IconDatabasePlus />
          </MuiIconButton>
          <MuiIconButton disabled={!isRoot} onClick={handleOpen}>
            <IconDatabaseCog />
          </MuiIconButton>
          {/* remove db */}
          <MuiIconButton disabled={!isRoot} onClick={handleRemoveDB}>
            <RemoveIcon />
          </MuiIconButton>
          {/* refresh tree */}
          <MuiIconButton disabled={!isRoot} onClick={handleRefresh}>
            <IconRefresh />
          </MuiIconButton>
        </Stack>
      </ToolbarBox>
    </>
  );
}
