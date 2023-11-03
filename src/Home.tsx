import { MemoDataset, PageProvider } from "@/components/Dataset";
import { FileTab, FileTabList, FileTabPanel } from "@/components/FileTabs";
import FileTreeView from "@/components/FileTree";
import { Content, Layout, Sidebar } from "@/components/Layout";
import { SideToolbar } from "@/components/SideToolbar";
import { FileNode, useDBStore } from "@/stores/db";
import { DTableType } from "@/stores/store";
import { useTabsStore } from "@/stores/tabs";
import TabContext from "@mui/lab/TabContext";

import CloseIcon from "@mui/icons-material/Close";
import { Box, BoxProps, IconButton } from "@mui/material";
import { styled } from "@mui/material/styles";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/primitives";
import { useEffect, useMemo, useState } from "react";

import { useResize } from "@/hooks";
import classes from "@/hooks/resize.module.css";

export const DatasetEmpty = styled((props) => <Box {...props} />)<BoxProps>(
  ({}) => ({
    display: "flex",
    marginTop: "20%",
    height: "100%",
    justifyContent: "center",
  })
);

function Home() {
  const [selectedTable, setSelectedTable] = useState<DTableType | null>(null);
  const dbList = useDBStore((state) => state.dbList);
  const appendDB = useDBStore((state) => state.append);
  const size = useDBStore((state) => state.size);
  const setSize = useDBStore((state) => state.setSize);
  const tabs = useTabsStore((state) => state.tabs);
  const activateTab = useTabsStore((state) => state.active);
  const removeTab = useTabsStore((state) => state.remove);
  const currentTab = useTabsStore((state) => state.table);

  async function openDirectory(name?: string) {
    const fileTree: FileNode = await invoke("get_folder_tree", { name });
    if (!!fileTree) {
      appendDB({
        data: fileTree,
      });
    }
  }
  async function openUrl() {
    const path: string = await invoke("opened_urls");
    console.log(path);
  }

  useEffect(() => {
    openUrl();
    const unlisten = listen("open-directory", (e) => {
      console.log(e.payload);

      openDirectory(e.payload as string);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const tabList = useMemo(() => {
    return (
      <FileTabList
        variant="scrollable"
        scrollButtons="auto"
        onChange={(_, value) => activateTab(value)}
      >
        {tabs.map((tab) => {
          return (
            <FileTab
              key={tab.id}
              value={tab.id}
              label={
                <Box
                  sx={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                >
                  <Box>{tab?.tableName}</Box>
                  <IconButton
                    size="small"
                    component="div"
                    onClick={(e) => {
                      e.stopPropagation();
                      removeTab(tab.id);
                    }}
                  >
                    <CloseIcon fontSize="inherit" />
                  </IconButton>
                </Box>
              }
            />
          );
        })}
      </FileTabList>
    );
  }, [tabs]);

  const items = useMemo(() => {
    return tabs.map((tab) => {
      return (
        <PageProvider key={tab.id} table={tab}>
          <FileTabPanel value={tab.id}>
            <MemoDataset />
          </FileTabPanel>
        </PageProvider>
      );
    });
  }, [tabs]);

  const [targetRefLeft, sizeLeft, actionLeft] = useResize(
    size,
    "left",
    setSize
  );

  return (
    <Layout>
      <Box
        ref={targetRefLeft}
        className={classes.sideBar}
        sx={{ width: sizeLeft + "px" }}
      >
        <Sidebar>
          <SideToolbar selectedTable={selectedTable}/>
          <TreeViewWrapper>
            {dbList.map((db, i) => (
              <FileTreeView
                key={i}
                rootKey={i}
                db={db}
                selected={
                  selectedTable?.rootKey == i ? selectedTable.tableName : null
                }
                onSelectTable={setSelectedTable}
              />
            ))}
          </TreeViewWrapper>
        </Sidebar>
        <div className={classes.controls}>
          <div className={classes.resizeVertical} onMouseDown={actionLeft} />
        </div>
      </Box>
      <Content sx={{ ml: `${sizeLeft}px` }}>
        <TabContext value={currentTab?.id ?? ""}>
          <Box>{tabs?.length > 0 ? tabList : <DatasetEmpty />}</Box>
          <Box>{items}</Box>
        </TabContext>
      </Content>
    </Layout>
  );
}

const TreeViewWrapper = styled(Box)<BoxProps>(({}) => ({
  width: "100%",
  maxHeight: "calc(100vh - 64px)",
  height: "calc(100vh - 64px)",
  overflow: "auto",
  pr: 1,
  pb: 2,
}));

export default Home;
